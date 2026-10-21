// The Graph tab: the recipe as Grasshopper-style components and wires.
//
// Settings are sliders on the left, every step is a component with its pipe stages listed
// inside it, and every name an input reads is a wire. Whatever you do here is a command that
// edits the recipe text (crates/core/src/graph.rs): typing or wiring an input, adding,
// removing or renaming a component, switching a preview, adding or changing a slider. The
// recipe then runs as usual, so the text stays the project, and the Recipe tab, the AI and this
// graph always show the same thing.
//
// Components are laid out left to right by what feeds what. Where you drag them is remembered
// on this device (view state, like the camera).

import { el, numberField, parseNumber } from './dom.js';
import { UNIT_LABELS, autoStep, listValue } from './settings.js';

const SVG = 'http://www.w3.org/2000/svg';
const COLUMN_GAP = 70;
const ROW_GAP = 16;
const STORE_KEY = 'easycad.graph.positions';
const GROUPS = ['Numbers & lists', 'Fields', 'Points & planes', 'Curves', 'Transforms', 'Surfaces', 'Toolpaths'];

/**
 * @typedef {{ id: string, kind: 'param' | 'step' | 'print' | 'export', name: string, line: number,
 *   deps: string[], el: HTMLElement, x: number, y: number, w: number, h: number }} GNode
 */

/**
 * @param {{
 *   canvas: HTMLElement, world: HTMLElement, wires: SVGSVGElement, inspector: HTMLElement,
 *   note: HTMLElement, empty: HTMLElement, palette: HTMLElement, search: HTMLInputElement,
 *   paletteList: HTMLElement, paletteFor: HTMLElement, undoButton: HTMLButtonElement,
 *   engine: ReturnType<import('./engine.js').createEngine>,
 *   command: (cmd: Record<string, any>) => Promise<string | null>,
 *   undo: () => Promise<boolean>,
 *   select: (name: string) => void,
 *   setValue: (name: string, value: any, quality: 'preview' | 'final') => void,
 * }} deps
 */
export function createGraph(deps) {
  const { canvas, world, wires, inspector } = deps;

  /** @type {any} the engine's graph of the recipe (easycad_core::graph::Graph) */
  let graph = null;
  /** @type {any} the last run (engine.js RunResult) */
  let result = null;
  let structure = '';
  let selected = '';
  let visible = false;
  let dirty = true;
  let project = '';
  let fitted = false;
  /** @type {Map<string, GNode>} */
  let nodes = new Map();
  const view = { x: 16, y: 16, k: 1 };
  /** @type {any[] | null} */
  let palette = null;
  /** @type {{ from?: string, input?: string, stage?: string } } */
  let paletteContext = {};
  /** Positions you dragged components to, per project: { [project]: { [id]: [x, y] } }. */
  let positions = loadPositions();

  // ------------------------------------------------------------------------------- updates

  /** A new run: fetch the graph of its recipe and redraw what changed. */
  async function update(r) {
    result = r;
    if ((r.project || '') !== project) {
      project = r.project || '';
      fitted = false;
    }
    let g = null;
    let problem = '';
    try {
      g = await deps.engine.graph(r.source);
    } catch (err) {
      problem = String(err.message || err);
    }
    if (r !== result) return; // a newer run arrived meanwhile
    graph = g;
    const key = JSON.stringify([
      g,
      problem,
      r.params.map((p) => [p.name, p.label, p.kind, p.unit, p.min, p.max, p.step, p.choices]),
    ]);
    const structural = key !== structure;
    if (structural) {
      structure = key;
      dirty = true;
    }
    deps.empty.hidden = !problem;
    deps.empty.textContent = problem ? `The recipe has a syntax error, so the graph can't show it: ${problem}. Fix it in the Recipe tab.` : '';
    if (!visible) return;
    if (dirty) render();
    else refresh();
    // Only a new structure rebuilds the inspector; a new run just updates what it shows, so a
    // field you're typing in stays put.
    if (structural) renderInspector();
    else refreshInspector();
  }

  function show(on) {
    visible = on;
    if (!on) {
      closePalette();
      return;
    }
    if (dirty) render();
    renderInspector();
    if (!fitted) fit(true);
    // Coming back: the selected component is highlighted again.
    if (selected && graph && graph.steps.some((s) => s.name === selected)) deps.select(selected);
  }

  function message(text, bad = false) {
    deps.note.textContent = text;
    deps.note.className = bad ? 'error' : 'hint';
    deps.note.hidden = !text;
  }

  /**
   * Runs a graph command. A refusal is shown under the toolbar, not thrown.
   * @returns {Promise<{ ok: boolean, name: string | null }>} name: what the command added
   */
  async function run(cmd) {
    message('');
    try {
      return { ok: true, name: await deps.command(cmd) };
    } catch (err) {
      message(String(err.message || err), true);
      return { ok: false, name: null };
    }
  }

  // ------------------------------------------------------------------------------- drawing

  /** Everything the run says about a step: status, summary, and its error messages. */
  function stepInfo(step) {
    const info = result.steps.find((s) => s.name === step.name) || { status: 'ok', summary: '' };
    const errors = result.diagnostics.filter((d) => d.severity === 'error' && d.line >= step.line && d.line <= step.endLine);
    return { status: errors.length ? 'error' : info.status, summary: info.summary, errors: errors.map((d) => d.message) };
  }

  function render() {
    dirty = false;
    world.replaceChildren(wires);
    wires.replaceChildren();
    nodes = new Map();
    if (!graph || !result) return;
    const known = new Set([...graph.params.map((p) => p.name), ...graph.steps.map((s) => s.name)]);
    const add = (/** @type {Omit<GNode, 'x' | 'y' | 'w' | 'h'>} */ n) => {
      world.append(n.el);
      nodes.set(n.id, { ...n, x: 0, y: 0, w: 0, h: 0 });
    };
    for (const p of graph.params) {
      const info = result.params.find((q) => q.name === p.name);
      if (info) add({ id: p.name, kind: 'param', name: p.name, line: p.line, deps: [], el: paramNode(info) });
    }
    for (const s of graph.steps) {
      const refs = new Set();
      for (const st of s.stages) {
        st.refs.forEach((r) => refs.add(r));
        st.inputs.forEach((i) => i.refs.forEach((r) => refs.add(r)));
      }
      add({ id: s.name, kind: 'step', name: s.name, line: s.line, deps: [...refs].filter((r) => known.has(r)), el: stepNode(s) });
    }
    graph.outputs.forEach((o, i) => {
      const id = `${o.kind}:${i}`;
      add({ id, kind: o.kind, name: id, line: o.line, deps: o.refs.filter((r) => known.has(r)), el: sinkNode(o) });
    });
    for (const n of nodes.values()) {
      n.w = n.el.offsetWidth;
      n.h = n.el.offsetHeight;
    }
    layout();
    for (const n of nodes.values()) {
      n.el.style.transform = `translate(${n.x}px, ${n.y}px)`;
      n.el.classList.toggle('selected', n.id === selected);
    }
    drawWires();
    applyView();
  }

  /** Values and statuses only (the recipe's structure didn't change): no rebuild. */
  function refresh() {
    if (!graph || !result) return;
    for (const s of graph.steps) {
      const n = nodes.get(s.name);
      if (!n) continue;
      const info = stepInfo(s);
      n.el.classList.toggle('error', info.status === 'error');
      n.el.classList.toggle('skipped', info.status === 'skipped');
      const foot = n.el.querySelector('.gnode-foot');
      if (foot) foot.textContent = info.errors[0] || info.summary;
    }
    for (const p of result.params) {
      const n = nodes.get(p.name);
      const input = /** @type {HTMLInputElement | null} */ (n && n.el.querySelector('input'));
      if (input && document.activeElement !== input) {
        if (input.type === 'checkbox') input.checked = p.value === true;
        else input.value = p.kind === 'list' ? valueText(p) : String(p.value);
      }
      const out = n && n.el.querySelector('output');
      if (out) out.textContent = valueText(p);
    }
  }

  function valueText(p) {
    if (p.kind === 'list') return JSON.stringify(p.value);
    if (p.kind !== 'number') return String(p.value);
    const unit = UNIT_LABELS[p.unit] ?? p.unit;
    return `${p.value}${unit === '°' ? '°' : unit ? ` ${unit}` : ''}`;
  }

  function port(kind, data = {}) {
    return el('span', { className: `gport ${kind}`, 'aria-hidden': 'true', ...data });
  }

  /** A setting: a slider (or toggle, dropdown, text field) with an output. */
  function paramNode(p) {
    const head = el('div', { className: 'gnode-head' }, [
      el('span', { className: 'gnode-title', text: p.label || p.name }),
      port('out', { 'data-out': p.name }),
    ]);
    const node = el('div', { className: 'gnode param', 'data-id': p.name, tabIndex: 0, title: `${p.name} (setting)` }, [head]);
    const body = el('div', { className: 'gslider' });
    if (p.kind === 'number' && p.min != null && p.max != null) {
      const range = /** @type {HTMLInputElement} */ (
        el('input', {
          type: 'range',
          min: String(p.min),
          max: String(p.max),
          step: String(p.step ?? autoStep(p.min, p.max)),
          value: String(p.value),
          'aria-label': p.label || p.name,
        })
      );
      const out = el('output', { text: valueText(p) });
      range.addEventListener('input', () => {
        out.textContent = valueText({ ...p, value: Number(range.value) });
        deps.setValue(p.name, Number(range.value), 'preview');
      });
      range.addEventListener('change', () => deps.setValue(p.name, Number(range.value), 'final'));
      body.append(range, out);
    } else if (p.kind === 'toggle') {
      const box = /** @type {HTMLInputElement} */ (el('input', { type: 'checkbox', checked: p.value === true, 'aria-label': p.label || p.name }));
      box.addEventListener('change', () => deps.setValue(p.name, box.checked, 'final'));
      body.append(box);
    } else if (p.kind === 'choice') {
      const select = /** @type {HTMLSelectElement} */ (el('select', { 'aria-label': p.label || p.name }, p.choices.map((c) => el('option', { value: String(c), text: String(c) }))));
      select.value = String(p.value);
      select.addEventListener('change', () => deps.setValue(p.name, p.choices.find((c) => String(c) === select.value) ?? select.value, 'final'));
      body.append(select);
    } else {
      const input = p.kind === 'number' ? numberField({ value: valueText(p), 'aria-label': p.label || p.name }, true).input : /** @type {HTMLInputElement} */ (el('input', { type: 'text', value: valueText(p), 'aria-label': p.label || p.name }));
      input.addEventListener('change', () => {
        if (p.kind === 'list') {
          // Typed as a list, [[0, 30], [1, 28]]; anything else puts the old value back.
          const r = listValue(p, input.value);
          if ('value' in r) deps.setValue(p.name, r.value, 'final');
          else input.value = valueText(p);
          return;
        }
        const v = p.kind === 'number' ? parseNumber(input.value) : input.value;
        if (p.kind !== 'number' || Number.isFinite(v)) deps.setValue(p.name, v, 'final');
      });
      body.append(input);
    }
    node.append(body);
    return node;
  }

  /** A step: header with its name, preview switch and output; then its stages and inputs. */
  function stepNode(s) {
    const info = stepInfo(s);
    const shown = graph.shown.includes(s.name);
    const eye = el('button', {
      type: 'button',
      className: 'gnode-eye',
      'aria-pressed': String(shown),
      'aria-label': shown ? `Hide ${s.name} in the 3D view` : `Show ${s.name} in the 3D view`,
      title: shown ? 'Shown in the 3D view (tap to hide)' : 'Not shown (tap to show)',
      text: shown ? '●' : '○',
    });
    eye.addEventListener('click', (e) => {
      e.stopPropagation();
      run({ op: 'setShown', step: s.name, shown: !shown });
    });
    const head = el('div', { className: 'gnode-head' }, [el('span', { className: 'gnode-title', text: s.name }), eye, port('out', { 'data-out': s.name })]);
    const node = el('div', { className: `gnode step ${info.status}`, 'data-id': s.name, tabIndex: 0 }, [head]);
    s.stages.forEach((st, k) => {
      if (st.node == null) {
        node.append(
          el('div', { className: 'grow expr', 'data-stage': String(k), 'data-refs': st.refs.join(','), title: st.text }, [
            port('in'),
            el('span', { className: 'grow-text', text: `= ${st.text}` }),
          ]),
        );
        return;
      }
      node.append(
        el('div', { className: `gstage${st.known ? '' : ' unknown'}`, title: st.doc || `${st.node} isn't a step the engine knows` }, [
          el('span', { text: k === 0 ? st.node : `|> ${st.node}` }),
        ]),
      );
      for (const input of st.inputs) {
        if (input.piped || (!input.text && !input.required && !input.unknown)) continue;
        const missing = input.required && !input.text;
        const wired = input.text && input.refs.length === 1 && input.text === input.refs[0];
        node.append(
          el(
            'div',
            {
              className: `grow${missing ? ' missing' : ''}${input.unknown ? ' unknown' : ''}`,
              'data-stage': String(k),
              'data-input': input.name,
              'data-refs': input.refs.join(','),
              title: `${input.name}: ${input.kind}${input.default ? ` = ${input.default}` : ''}${input.doc ? ` · ${input.doc}` : ''}`,
            },
            [
              port('in'),
              el('span', { className: 'grow-name', text: input.name }),
              el('span', { className: 'grow-text', text: missing ? 'missing' : wired ? '' : input.text }),
            ],
          ),
        );
      }
    });
    node.append(el('div', { className: 'gnode-foot', text: info.errors[0] || info.summary }));
    return node;
  }

  /** `print` and `export`: where the recipe's results go. */
  function sinkNode(o) {
    const title = o.kind === 'print' ? 'Print (G-code)' : `Export ${o.file || ''}`;
    return el('div', { className: `gnode sink ${o.kind}`, 'data-id': `${o.kind}`, title: `${o.kind} ${o.text}` }, [
      el('div', { className: 'gnode-head' }, [el('span', { className: 'gnode-title', text: title })]),
      el('div', { className: 'grow', 'data-refs': o.refs.join(',') }, [port('in'), el('span', { className: 'grow-text', text: o.text })]),
    ]);
  }

  /** Columns by what feeds what (settings first, outputs last), dragged positions kept. */
  function layout() {
    const col = new Map();
    const columnOf = (/** @type {GNode} */ n, depth = 0) => {
      if (col.has(n.id)) return col.get(n.id);
      if (depth > 500) return 1;
      let c = n.kind === 'param' ? 0 : 1;
      for (const d of n.deps) {
        const m = nodes.get(d);
        if (m && m !== n) c = Math.max(c, columnOf(m, depth + 1) + 1);
      }
      col.set(n.id, c);
      return c;
    };
    const steps = [...nodes.values()].filter((n) => n.kind !== 'print' && n.kind !== 'export');
    steps.forEach((n) => columnOf(n));
    const last = Math.max(0, ...steps.map((n) => col.get(n.id)));
    for (const n of nodes.values()) if (!col.has(n.id)) col.set(n.id, last + 1);

    /** @type {GNode[][]} */
    const columns = [];
    for (const n of nodes.values()) (columns[col.get(n.id)] ||= []).push(n);
    const widths = columns.map((c) => Math.max(0, ...(c || []).map((n) => n.w)));
    let x = 0;
    const saved = positions[project] || {};
    columns.forEach((list, c) => {
      if (!list) return;
      // Each component near the components that feed it (fewer crossing wires).
      const centre = (/** @type {GNode} */ n) => {
        const ys = n.deps.map((d) => nodes.get(d)).filter((m) => m && col.get(m.id) < c).map((m) => m.y + m.h / 2);
        return ys.length ? ys.reduce((a, b) => a + b, 0) / ys.length : Infinity;
      };
      if (c > 0) list.sort((a, b) => centre(a) - centre(b) || a.line - b.line);
      else list.sort((a, b) => a.line - b.line);
      let y = 0;
      for (const n of list) {
        const pinned = saved[n.id];
        n.x = pinned ? pinned[0] : x;
        n.y = pinned ? pinned[1] : y;
        if (!pinned) y += n.h + ROW_GAP;
      }
      x += (widths[c] || 0) + COLUMN_GAP;
    });
  }

  /** Where a port is, in graph coordinates. */
  function portAt(/** @type {GNode} */ n, /** @type {HTMLElement} */ row, side) {
    const ox = side === 'out' ? n.w : 0;
    let oy = row.offsetTop + row.offsetHeight / 2;
    for (let p = /** @type {HTMLElement | null} */ (row.offsetParent); p && p !== n.el; p = /** @type {HTMLElement | null} */ (p.offsetParent)) oy += p.offsetTop;
    return [n.x + ox, n.y + oy];
  }

  function wirePath(a, b) {
    const dx = Math.max(30, Math.abs(b[0] - a[0]) / 2);
    return `M ${a[0]} ${a[1]} C ${a[0] + dx} ${a[1]}, ${b[0] - dx} ${b[1]}, ${b[0]} ${b[1]}`;
  }

  function drawWires() {
    wires.replaceChildren();
    let maxX = 0;
    let maxY = 0;
    for (const n of nodes.values()) {
      maxX = Math.max(maxX, n.x + n.w);
      maxY = Math.max(maxY, n.y + n.h);
      for (const row of /** @type {NodeListOf<HTMLElement>} */ (n.el.querySelectorAll('[data-refs]'))) {
        for (const ref of (row.dataset.refs || '').split(',')) {
          const from = nodes.get(ref);
          if (!from || from === n) continue;
          const head = /** @type {HTMLElement} */ (from.el.querySelector('.gnode-head'));
          const path = document.createElementNS(SVG, 'path');
          path.setAttribute('d', wirePath(portAt(from, head, 'out'), portAt(n, row, 'in')));
          path.setAttribute('class', `gwire${from.id === selected || n.id === selected ? ' active' : ''}`);
          wires.append(path);
        }
      }
    }
    wires.setAttribute('width', String(maxX + 40));
    wires.setAttribute('height', String(maxY + 40));
  }

  // ------------------------------------------------------------------------------- view

  function applyView() {
    world.style.transform = `translate(${view.x}px, ${view.y}px) scale(${view.k})`;
  }

  /**
   * Frames every component. `readable` (the first view of a project) doesn't zoom out past
   * where text can be read: a wide graph on a phone then starts at its left end, the
   * sliders, and you pan along it.
   */
  function fit(readable = false) {
    if (!nodes.size) return;
    const xs = [...nodes.values()];
    const minX = Math.min(...xs.map((n) => n.x));
    const minY = Math.min(...xs.map((n) => n.y));
    const w = Math.max(...xs.map((n) => n.x + n.w)) - minX;
    const h = Math.max(...xs.map((n) => n.y + n.h)) - minY;
    const fits = Math.min(1, (canvas.clientWidth - 24) / Math.max(1, w), (canvas.clientHeight - 24) / Math.max(1, h));
    const k = Math.max(readable ? 0.6 : 0.12, fits);
    view.k = k;
    view.x = w * k > canvas.clientWidth - 24 ? 12 - minX * k : (canvas.clientWidth - w * k) / 2 - minX * k;
    view.y = Math.max(12, (canvas.clientHeight - h * k) / 2) - minY * k;
    fitted = true;
    applyView();
  }

  function zoomAt(cx, cy, factor) {
    const k = Math.max(0.2, Math.min(2.5, view.k * factor));
    view.x = cx - ((cx - view.x) * k) / view.k;
    view.y = cy - ((cy - view.y) * k) / view.k;
    view.k = k;
    applyView();
  }

  /** A pointer position in graph coordinates. */
  function toGraph(e) {
    const r = canvas.getBoundingClientRect();
    return [(e.clientX - r.left - view.x) / view.k, (e.clientY - r.top - view.y) / view.k];
  }

  // ------------------------------------------------------------------------------- pointers

  /** @type {Map<number, { x: number, y: number }>} */
  const pointers = new Map();
  /** @type {null | { kind: 'pan' | 'pinch' | 'move' | 'wire', id?: string, from?: string, start: number[], at: number[], moved: boolean, temp?: SVGPathElement, dist?: number }} */
  let drag = null;

  canvas.addEventListener('pointerdown', (e) => {
    const target = /** @type {HTMLElement} */ (e.target);
    if (target.closest('input, select, button, output')) return;
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (pointers.size === 2) {
      const [a, b] = [...pointers.values()];
      drag = { kind: 'pinch', start: [e.clientX, e.clientY], at: [e.clientX, e.clientY], moved: true, dist: Math.hypot(a.x - b.x, a.y - b.y) };
      return;
    }
    const out = /** @type {HTMLElement | null} */ (target.closest('.gport.out'));
    const node = /** @type {HTMLElement | null} */ (target.closest('.gnode'));
    if (out && node) {
      const temp = /** @type {SVGPathElement} */ (document.createElementNS(SVG, 'path'));
      temp.setAttribute('class', 'gwire temp');
      wires.append(temp);
      drag = { kind: 'wire', from: out.dataset.out, start: toGraph(e), at: [e.clientX, e.clientY], moved: false, temp };
    } else if (node) {
      drag = { kind: 'move', id: node.dataset.id, start: toGraph(e), at: [e.clientX, e.clientY], moved: false };
    } else {
      drag = { kind: 'pan', start: [view.x, view.y], at: [e.clientX, e.clientY], moved: false };
    }
    canvas.setPointerCapture(e.pointerId);
  });

  canvas.addEventListener('pointermove', (e) => {
    if (!pointers.has(e.pointerId) || !drag) return;
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    const dx = e.clientX - drag.at[0];
    const dy = e.clientY - drag.at[1];
    if (Math.hypot(dx, dy) > 4) drag.moved = true;
    if (drag.kind === 'pinch' && pointers.size === 2) {
      const [a, b] = [...pointers.values()];
      const dist = Math.hypot(a.x - b.x, a.y - b.y);
      const r = canvas.getBoundingClientRect();
      zoomAt((a.x + b.x) / 2 - r.left, (a.y + b.y) / 2 - r.top, dist / (drag.dist || dist));
      drag.dist = dist;
    } else if (drag.kind === 'pan') {
      view.x = drag.start[0] + dx;
      view.y = drag.start[1] + dy;
      applyView();
    } else if (drag.kind === 'move' && drag.moved) {
      const n = nodes.get(drag.id || '');
      if (!n) return;
      const p = toGraph(e);
      n.x += p[0] - drag.start[0];
      n.y += p[1] - drag.start[1];
      drag.start = p;
      n.el.style.transform = `translate(${n.x}px, ${n.y}px)`;
      drawWires();
    } else if (drag.kind === 'wire' && drag.temp) {
      const from = nodes.get(drag.from || '');
      if (!from) return;
      const head = /** @type {HTMLElement} */ (from.el.querySelector('.gnode-head'));
      drag.temp.setAttribute('d', wirePath(portAt(from, head, 'out'), toGraph(e)));
      if (!drag.temp.isConnected) wires.append(drag.temp);
    }
  });

  const endDrag = (e) => {
    pointers.delete(e.pointerId);
    const d = drag;
    if (!d || (d.kind === 'pinch' && pointers.size)) return;
    drag = null;
    if (d.kind === 'move') {
      const n = nodes.get(d.id || '');
      if (!n) return;
      if (d.moved) {
        (positions[project] ||= {})[n.id] = [Math.round(n.x), Math.round(n.y)];
        savePositions();
      } else if (n.kind === 'param' || n.kind === 'step') {
        select(n.id);
      }
    } else if (d.kind === 'pan' && !d.moved) {
      select('');
    } else if (d.kind === 'wire') {
      d.temp?.remove();
      if (!d.moved) {
        select(d.from || '');
        return;
      }
      const hit = /** @type {HTMLElement | null} */ (document.elementFromPoint(e.clientX, e.clientY));
      const row = /** @type {HTMLElement | null} */ (hit && hit.closest('.grow[data-input]'));
      const target = row && /** @type {HTMLElement} */ (row.closest('.gnode'));
      if (row && target) {
        run({ op: 'setInput', step: target.dataset.id, stage: Number(row.dataset.stage), input: row.dataset.input, text: d.from });
      } else if (hit && hit.closest('.gnode')) {
        message('Drop the wire on an input (a row with a dot on its left). To wire an input that isn’t shown, select the component and pick the source in its list below.');
      } else {
        openPalette({ from: d.from });
      }
    }
  };
  canvas.addEventListener('pointerup', endDrag);
  canvas.addEventListener('pointercancel', (e) => {
    pointers.delete(e.pointerId);
    drag?.temp?.remove();
    drag = null;
  });

  canvas.addEventListener(
    'wheel',
    (e) => {
      e.preventDefault();
      const r = canvas.getBoundingClientRect();
      zoomAt(e.clientX - r.left, e.clientY - r.top, Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.0015)));
    },
    { passive: false },
  );
  canvas.addEventListener('dblclick', (e) => {
    if (!(/** @type {HTMLElement} */ (e.target).closest('.gnode'))) openPalette({});
  });
  canvas.addEventListener('keydown', (e) => {
    const t = /** @type {HTMLElement} */ (e.target);
    if (t.closest('input, select, textarea')) return;
    if (e.key === 'Enter' && t.dataset.id) select(t.dataset.id);
    if (e.key === 'Delete' && selected) {
      e.preventDefault();
      remove(selected);
    }
    if (e.key === 'z' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      deps.undo();
    }
  });

  // ------------------------------------------------------------------------------- selection

  function select(id) {
    selected = id;
    for (const n of nodes.values()) n.el.classList.toggle('selected', n.id === id);
    drawWires();
    renderInspector();
    deps.select(graph && graph.steps.some((s) => s.name === id) ? id : '');
  }

  async function remove(name) {
    const wasSelected = selected === name;
    await run({ op: 'remove', name });
    if (wasSelected) select('');
  }

  // ------------------------------------------------------------------------------- inspector

  /** Every name a wire can come from. */
  function sources(except) {
    if (!graph) return [];
    return [...graph.params.map((p) => p.name), ...graph.steps.map((s) => s.name)].filter((n) => n !== except);
  }

  function renderInspector() {
    const focused = /** @type {HTMLElement | null} */ (inspector.contains(document.activeElement) ? document.activeElement : null);
    const focusKey = focused?.dataset.focus;
    inspector.replaceChildren();
    try {
      fillInspector();
    } finally {
      const again = focusKey && /** @type {HTMLInputElement | null} */ (inspector.querySelector(`[data-focus="${CSS.escape(focusKey)}"]`));
      if (again) {
        again.focus();
        if (again.setSelectionRange && again.type === 'text') again.setSelectionRange(again.value.length, again.value.length);
      }
    }
  }

  /** Updates the live parts of the inspector after a run: status, summary, errors, values. */
  function refreshInspector() {
    if (!graph || !result) return;
    const step = graph.steps.find((s) => s.name === selected);
    if (step) {
      const info = stepInfo(step);
      const badge = inspector.querySelector('[data-live="badge"]');
      if (badge) {
        badge.className = `badge ${info.status}`;
        badge.textContent = info.status;
      }
      const summary = inspector.querySelector('[data-live="summary"]');
      if (summary) summary.textContent = info.summary;
      const errors = inspector.querySelector('[data-live="errors"]');
      if (errors) errors.replaceChildren(...info.errors.map((m) => el('p', { className: 'error', text: m })));
      return;
    }
    const param = result.params.find((p) => p.name === selected);
    const value = /** @type {HTMLInputElement | null} */ (inspector.querySelector('[data-key="value"]'));
    if (param && value && document.activeElement !== value) value.value = String(param.value);
  }

  function fillInspector() {
    if (!graph || !result) return;
    const step = graph.steps.find((s) => s.name === selected);
    const param = result.params.find((p) => p.name === selected);
    if (step) inspectStep(step);
    else if (param) inspectParam(param);
    else
      inspector.append(
        el('p', {
          className: 'hint',
          text: 'Tap a component to see and edit all its inputs here. Drag from a component’s right edge to an input to wire it; drop a wire on empty space to add a component fed by it.',
        }),
      );
  }

  /** A name field that renames the component (and everything that reads it) on change. */
  function nameField(name) {
    const input = /** @type {HTMLInputElement} */ (
      el('input', { type: 'text', value: name, className: 'ginsp-name', 'aria-label': 'Name', spellcheck: false, autocapitalize: 'off', 'data-focus': 'name' })
    );
    input.addEventListener('change', async () => {
      const to = input.value.trim();
      if (!to || to === name) {
        input.value = name;
        return;
      }
      if (!(await run({ op: 'rename', name, to })).ok) {
        input.value = name;
        return;
      }
      // The component keeps its place and its selection under the new name.
      const saved = positions[project];
      if (saved && saved[name]) {
        saved[to] = saved[name];
        delete saved[name];
        savePositions();
      }
      select(to);
    });
    return input;
  }

  function button(text, onClick, primary = false) {
    const b = el('button', { type: 'button', className: `btn${primary ? ' primary' : ''}`, text });
    b.addEventListener('click', onClick);
    return b;
  }

  function inspectStep(step) {
    const info = stepInfo(step);
    const shown = graph.shown.includes(step.name);
    inspector.append(
      el('div', { className: 'ginsp-head' }, [nameField(step.name), el('span', { className: `badge ${info.status}`, text: info.status, 'data-live': 'badge' })]),
      el('p', { className: 'hint first', text: info.summary, 'data-live': 'summary' }),
      el('div', { 'data-live': 'errors' }, info.errors.map((m) => el('p', { className: 'error', text: m }))),
      el('div', { className: 'buttons tight' }, [
        button(shown ? 'Hide in 3D' : 'Show in 3D', () => run({ op: 'setShown', step: step.name, shown: !shown })),
        button('+ Next component', () => openPalette({ from: step.name })),
        button('+ Stage |>', () => openPalette({ stage: step.name })),
        button('Remove', () => remove(step.name)),
      ]),
    );
    step.stages.forEach((st, k) => {
      const box = el('div', { className: 'ginsp-stage' });
      if (st.node == null) {
        box.append(el('h3', { text: k === 0 ? 'Expression' : `Stage ${k + 1}` }));
        const input = textField(st.text, 'An expression: numbers, names, + − × ÷, calls');
        input.dataset.focus = `expr:${k}`;
        input.addEventListener('change', () => run({ op: 'setExpr', step: step.name, stage: k, text: input.value }));
        box.append(input, el('p', { className: 'hint', text: `Reads: ${st.refs.join(', ') || 'nothing'}` }));
        inspector.append(box);
        return;
      }
      const title = el('h3', { text: k === 0 ? st.node : `|> ${st.node}` });
      if (st.group) title.append(' ', el('small', { text: st.group }));
      box.append(title);
      if (st.doc) box.append(el('p', { className: 'hint', text: st.doc }));
      if (!st.known) box.append(el('p', { className: 'error', text: `${st.node} isn’t a step the engine knows; see the catalog in the Recipe tab.` }));
      for (const input of st.inputs) box.append(inputField(step, k, input));
      if (k > 0) box.append(el('div', { className: 'buttons tight' }, [button('Remove this stage', () => run({ op: 'removeStage', step: step.name, stage: k }))]));
      inspector.append(box);
    });
  }

  function textField(value, placeholder) {
    return /** @type {HTMLInputElement} */ (
      el('input', { type: 'text', value, placeholder, spellcheck: false, autocapitalize: 'off', autocomplete: 'off', className: 'ginput-text' })
    );
  }

  function inputField(step, k, input) {
    const label = el('label', { text: input.name });
    label.append(' ', el('small', { text: `${input.kind}${input.default ? ` = ${input.default}` : input.required ? ' · required' : ''}` }));
    const row = el('div', { className: `ginput${input.unknown ? ' unknown' : ''}` }, [label]);
    if (input.piped) {
      row.append(el('p', { className: 'hint', text: '← from the stage above (|>)' }));
      return row;
    }
    if (input.unknown) {
      row.append(el('p', { className: 'error', text: `Not an input of this step: ${input.text}. Fix or remove it in the Recipe tab.` }));
      return row;
    }
    const text = textField(input.text || '', input.default ? `${input.default} (default)` : 'required: type a value or wire one');
    text.dataset.focus = `in:${k}:${input.name}`;
    const commit = (value) => run({ op: 'setInput', step: step.name, stage: k, input: input.name, text: value });
    text.addEventListener('change', () => commit(text.value));
    const wire = /** @type {HTMLSelectElement} */ (
      el('select', { 'aria-label': `Wire ${input.name} from` }, [
        el('option', { value: '', text: 'wire from…' }),
        ...sources(step.name).map((n) => el('option', { value: n, text: n })),
      ])
    );
    wire.addEventListener('change', () => wire.value && commit(wire.value));
    const clear = el('button', { type: 'button', className: 'btn', text: '×', title: 'Clear (back to the default)', 'aria-label': `Clear ${input.name}` });
    clear.addEventListener('click', () => commit(''));
    row.append(el('div', { className: 'ginput-row' }, [text, wire, input.text ? clear : null]));
    if (input.doc) row.append(el('small', { className: 'hint', text: input.doc }));
    return row;
  }

  function inspectParam(p) {
    const num = (label, value, key) => {
      const { input } = numberField({ value: value == null ? '' : String(value), 'data-key': key, 'data-focus': `p:${key}` });
      return el('label', { className: 'ginput' }, [el('span', { text: label }), input]);
    };
    const labelInput = /** @type {HTMLInputElement} */ (el('input', { type: 'text', value: p.label || '', 'data-key': 'label', 'data-focus': 'p:label' }));
    inspector.append(
      el('div', { className: 'ginsp-head' }, [nameField(p.name), el('span', { className: 'badge', text: 'setting' })]),
      el('p', { className: 'hint first', text: `A ${p.kind === 'number' ? 'number slider' : p.kind} in the Settings tab${p.unit ? `, in ${p.unit}` : ''}.` }),
    );
    if (p.kind === 'number') {
      const fields = el('div', { className: 'ginsp-grid' }, [
        el('label', { className: 'ginput' }, [el('span', { text: 'Label' }), labelInput]),
        num('Value', p.value, 'value'),
        num('Min', p.min, 'min'),
        num('Max', p.max, 'max'),
        num('Step', p.step, 'step'),
      ]);
      const apply = button(
        'Apply',
        () => {
          const get = (key) => /** @type {HTMLInputElement} */ (fields.querySelector(`[data-key="${key}"]`)).value.trim();
          const value = parseNumber(get('value'));
          const min = get('min') === '' ? Math.min(0, value) : parseNumber(get('min'));
          const max = get('max') === '' ? Math.max(value * 2, 1) : parseNumber(get('max'));
          const step = get('step') === '' ? null : parseNumber(get('step'));
          run({ op: 'setParam', name: p.name, value, min, max, step, label: get('label') || null });
        },
        true,
      );
      inspector.append(fields, el('div', { className: 'buttons tight' }, [apply, button('Remove', () => remove(p.name))]));
    } else {
      inspector.append(el('div', { className: 'buttons tight' }, [button('Remove', () => remove(p.name))]));
    }
  }

  // ------------------------------------------------------------------------------- palette

  async function openPalette(context) {
    paletteContext = context;
    if (!palette) {
      try {
        palette = await deps.engine.palette();
      } catch (err) {
        message(String(err.message || err), true);
        return;
      }
    }
    deps.paletteFor.textContent = context.stage
      ? `Adds a stage to ${context.stage}: its result goes into the new step's first input.`
      : context.from
        ? `Adds a component fed by ${context.from}.`
        : 'Adds a component. Wire its inputs afterwards.';
    deps.palette.hidden = false;
    deps.search.value = '';
    renderPalette('');
    deps.search.focus();
  }

  function closePalette() {
    deps.palette.hidden = true;
  }

  function renderPalette(query) {
    const q = query.trim().toLowerCase();
    const list = (palette || []).filter((n) => !q || n.name.includes(q) || n.doc.toLowerCase().includes(q));
    deps.paletteList.replaceChildren();
    for (const group of GROUPS) {
      const items = list.filter((n) => n.group === group);
      if (!items.length) continue;
      deps.paletteList.append(el('h3', { text: group }));
      for (const n of items) {
        const b = el('button', { type: 'button', className: 'gpick' }, [
          el('code', { text: `${n.name}(${n.inputs.map((i) => i.name).join(', ')})` }),
          el('span', { text: n.doc }),
        ]);
        b.addEventListener('click', () => pick(n.name));
        deps.paletteList.append(b);
      }
    }
    if (!deps.paletteList.childElementCount) deps.paletteList.append(el('p', { className: 'hint', text: 'No step matches. Try a Grasshopper name, like “tween curve” or “divide”.' }));
  }

  async function pick(node) {
    closePalette();
    const ctx = paletteContext;
    if (ctx.stage) {
      await run({ op: 'addStage', step: ctx.stage, node });
      select(ctx.stage);
      return;
    }
    const { name } = await run({ op: 'addStep', node, from: ctx.from || null, input: ctx.input || null });
    if (name) select(name);
  }

  deps.search.addEventListener('input', () => renderPalette(deps.search.value));
  deps.search.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') closePalette();
    if (e.key === 'Enter') {
      const first = /** @type {HTMLButtonElement | null} */ (deps.paletteList.querySelector('.gpick'));
      first?.click();
    }
  });

  // ------------------------------------------------------------------------------- storage

  function loadPositions() {
    try {
      const saved = JSON.parse(localStorage.getItem(STORE_KEY) || '{}');
      return saved && typeof saved === 'object' ? saved : {};
    } catch {
      return {};
    }
  }

  function savePositions() {
    try {
      localStorage.setItem(STORE_KEY, JSON.stringify(positions));
    } catch {
      // Storage unavailable: positions last until the page closes.
    }
  }

  return {
    update,
    show,
    fit,
    openPalette,
    closePalette,
    /** Forgets dragged positions and lays the graph out again. */
    tidy() {
      delete positions[project];
      savePositions();
      dirty = true;
      if (visible) {
        render();
        fit(true);
      }
    },
    /** Adds a number slider. */
    async addSlider() {
      const { name } = await run({ op: 'addParam', value: 1, min: 0, max: 10 });
      if (name) select(name);
    },
    message,
  };
}
