// The lamp builder's parts on screen (docs/lamp-builder.md): the Plan (the assembly's
// cross-section, every part as wide as its bead, its ports as `+` buttons), the dialog that
// adds a part, and the values a new part starts with. Nothing here edits the recipe: adding and
// removing go through the engine's commands (app.js).

import { el } from './dom.js';

const SVG = 'http://www.w3.org/2000/svg';
/** Colours for the parts, in order (EasyGCode's palette: blue, green, amber, violet, teal, pink). */
const COLORS = ['#4f9dff', '#2bd9a0', '#ffb454', '#b58cff', '#3fd0d4', '#ff7ab6'];
/** A section this long (mm) takes more than one part. */
const ROOMY = 60;
/** The `+` buttons' radius on screen, in pixels. */
const BUTTON_PX = 11;
/** Points per part drawn in the plan, at most. */
const PLAN_POINTS = 600;

/**
 * Which parts the engine says run into each other (`a` and `b` run into each other …).
 * @param {any[]} diagnostics
 * @returns {Set<string>}
 */
export function collidingParts(diagnostics) {
  const out = new Set();
  for (const d of diagnostics) {
    const m = /^`([^`]+)` and `([^`]+)` run into each other/.exec(d.message);
    if (m) {
      out.add(m[1]);
      out.add(m[2]);
    }
  }
  return out;
}

/**
 * Each printed part's toolpath as the plan draws it: (radius, height) where it's shown, thinned
 * out, with its bead width. Keyed by part (print) name.
 * @param {any} result the engine's result
 * @returns {Map<string, { points: number[][], width: number }>}
 */
export function sections(result) {
  const out = new Map();
  const { points, starts, widths } = result.paths;
  const owner = result.parts?.paths || [];
  for (let k = 0; k + 1 < starts.length; k++) {
    const print = result.prints[owner[k]];
    if (!print) continue;
    const [a, b] = [starts[k], starts[k + 1]];
    const step = Math.max(1, Math.ceil((b - a) / PLAN_POINTS));
    const entry = out.get(print.name) || { points: [], width: 0 };
    let wsum = 0;
    let wn = 0;
    for (let i = a; i < b; i += step) {
      entry.points.push([Math.hypot(points[3 * i], points[3 * i + 1]), points[3 * i + 2]]);
      if (widths && widths[i] > 0) {
        wsum += widths[i];
        wn++;
      }
    }
    entry.width = Math.max(entry.width, wn ? wsum / wn : 1);
    out.set(print.name, entry);
  }
  return out;
}

/**
 * Draws the plan: the assembly cut through its axis (both halves), each part in its colour (red
 * where it runs into another), its name beside it, and a `+` at every port with room.
 * @param {HTMLElement} host
 * @param {any} result
 * @param {{ onPort: (port: any) => void, onPart: (name: string) => void, selected?: string | null }} handlers
 */
export function renderPlan(host, result, handlers) {
  host.replaceChildren();
  const parts = sections(result);
  // Ports with room: not one a part is already mounted on, unless its section is long (a tube
  // takes several shades along it).
  const used = new Set(result.params.filter((p) => p.kind === 'port').map((p) => String(p.value)));
  const ports = (result.ports || []).filter((p) => p.value.length >= 4 && p.value[3] > 0 && (!used.has(p.name) || p.value[3] >= ROOMY));
  if (!parts.size) return;
  let [rmax, zmin, zmax] = [1, Infinity, -Infinity];
  for (const { points } of parts.values()) {
    for (const [r, z] of points) {
      rmax = Math.max(rmax, r);
      zmin = Math.min(zmin, z);
      zmax = Math.max(zmax, z);
    }
  }
  for (const p of ports) {
    rmax = Math.max(rmax, p.value[0]);
    zmin = Math.min(zmin, p.value[1]);
    zmax = Math.max(zmax, p.value[1]);
  }
  const pad = Math.max(12, 0.08 * Math.max(2 * rmax, zmax - zmin));
  const [x0, w] = [-rmax - pad, 2 * (rmax + pad)];
  const [y0, h] = [-(zmax + pad), zmax - zmin + 2 * pad];
  const svg = document.createElementNS(SVG, 'svg');
  svg.setAttribute('viewBox', `${x0} ${y0} ${w} ${h}`);
  svg.setAttribute('preserveAspectRatio', 'xMidYMid meet');
  svg.setAttribute('role', 'img');
  svg.setAttribute('aria-label', 'Plan: the parts cut through their axis');
  const node = (name, attrs, parent = svg) => {
    const n = document.createElementNS(SVG, name);
    for (const [k, v] of Object.entries(attrs)) n.setAttribute(k, String(v));
    parent.append(n);
    return n;
  };
  // The axis.
  node('line', { x1: 0, y1: -(zmax + pad / 2), x2: 0, y2: -(zmin - pad / 2), class: 'plan-axis' });
  const hit = collidingParts(result.diagnostics);
  // Model units per screen pixel, so labels and buttons keep their size on screen.
  const px = Math.max(w / Math.max(1, host.clientWidth), h / Math.max(1, host.clientHeight));
  const unit = BUTTON_PX * px;
  [...parts.entries()].forEach(([name, { points, width }], i) => {
    const g = node('g', { class: `plan-part${hit.has(name) ? ' collides' : ''}${handlers.selected === name ? ' selected' : ''}`, 'data-part': name });
    g.style.setProperty('--part', COLORS[i % COLORS.length]);
    for (const side of [1, -1]) {
      node('polyline', { points: points.map(([r, z]) => `${side * r},${-z}`).join(' '), 'stroke-width': width }, g);
    }
    // Its name beside its widest point.
    let far = points[0];
    for (const p of points) if (p[0] > far[0]) far = p;
    const label = node('text', { x: far[0] + width, y: -far[1], 'font-size': 13 * px, class: 'plan-label' }, g);
    label.textContent = name;
    g.addEventListener('click', () => handlers.onPart(name));
  });
  for (const port of ports) {
    // Outside ports a little further out, inside ones further in, and a little way into the
    // section from its end, so the two faces' buttons (and a shade's end and the next one's
    // start) stay apart.
    const [pr, pz, inward] = port.value;
    const side = port.value[5] < 0 ? -1 : 1;
    const r = pr + side * unit * 1.1;
    const z = pz + inward * unit * 1.2;
    const g = node('g', { class: 'plan-port', tabindex: 0, role: 'button', 'aria-label': `Add here: ${port.part} · ${port.label}` });
    node('line', { x1: pr, y1: -pz, x2: r, y2: -z, class: 'plan-port-stem', 'stroke-width': px * 1.5 }, g);
    node('circle', { cx: r, cy: -z, r: unit }, g);
    node('path', { d: `M${r - unit * 0.55},${-z}h${unit * 1.1}M${r},${-z - unit * 0.55}v${unit * 1.1}`, 'stroke-width': px * 2 }, g);
    const title = node('title', {}, g);
    title.textContent = `${port.part} · ${port.label}`;
    const go = () => handlers.onPort(port);
    g.addEventListener('click', go);
    g.addEventListener('keydown', (e) => {
      if (/** @type {KeyboardEvent} */ (e).key === 'Enter' || /** @type {KeyboardEvent} */ (e).key === ' ') go();
    });
  }
  host.append(svg);
}

/**
 * A free name for a new part, after its shape: `tube`, `tube2`, …
 * @param {string} shape
 * @param {string[]} taken
 */
export function newPartName(shape, taken) {
  const base = String(shape || 'part').toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_+|_+$/g, '') || 'part';
  const start = /^[a-z_]/.test(base) ? base : `part_${base}`;
  let name = start;
  for (let k = 2; taken.includes(name); k++) name = `${start}${k}`;
  return name;
}

/**
 * Sensible starting values for a new shade on a port, scaled to where it goes (the template's
 * own defaults are for a shade on an E27 socket, 23 mm round): outside, it opens out; inside, it
 * stays within. And which way it goes: on from the port's section (away from what it's on).
 * @param {string} shape
 * @param {number[]} port [radius, height, inward, length, socket, side (1 outside, -1 inside)]
 */
export function shadeDefaults(shape, port) {
  const r = Math.max(5, port[0]);
  const inside = port[5] < 0;
  const d = (v) => Math.max(20, Math.round(v));
  /** @type {Record<string, unknown>} */
  const values = { direction: port[2] < 0 ? 'up' : 'down' };
  // Outside it opens out (as the template's shade does from the socket); inside, it narrows.
  const out = (extra) => d(inside ? 2 * r - extra / 2 : 2 * r + extra);
  if (['cone', 'bell', 'dome', 'own'].includes(shape)) values.opening = out(104);
  if (['sphere', 'flat sphere'].includes(shape)) {
    values.sphere = d(2 * r + (inside ? 20 : 154));
    values.opening = out(104);
  }
  if (['bell in-out', 'cone in-out'].includes(shape)) values.widest = inside ? d(2 * r) : out(154);
  if (shape === 'own') values.own_r = d(inside ? r - 10 : r + 27);
  return values;
}

/**
 * The dialog that adds a part: where it goes (a port; the one tapped in the plan), then the
 * component's `@ask` settings (the shade's shape, which way it goes). Resolves with what to add,
 * or null.
 * @param {HTMLElement} host the dialog's element
 * @param {any} result
 * @param {any} [port] the port tapped, if any
 * @returns {Promise<{ component: string, port: string, values: Record<string, unknown> } | null>}
 */
export function askNewPart(host, result, port) {
  const component = (result.components || [])[0];
  const ports = (result.ports || []).filter((p) => p.value.length >= 4 && p.value[3] > 0);
  if (!component) return Promise.resolve(null);
  return new Promise((resolve) => {
    host.replaceChildren();
    const field = (label, control) => el('label', { className: 'setting choice' }, [el('span', { text: label }), control]);
    const where = /** @type {HTMLSelectElement} */ (
      el(
        'select',
        { id: 'addPartPort' },
        ports.map((p) => el('option', { value: p.name, text: `${p.part} · ${p.label}` })),
      )
    );
    if (port) where.value = port.name;
    const asks = component.ask.map((p) => {
      const select = /** @type {HTMLSelectElement} */ (
        el(
          'select',
          { id: `addPart-${p.name}` },
          p.choices.map((c) => el('option', { value: String(c), text: String(c) })),
        )
      );
      select.value = String(p.value);
      return { p, select };
    });
    const direction = asks.find((a) => a.p.name === 'direction');
    // On from the port's section, unless chosen otherwise.
    const follow = () => {
      const chosen = ports.find((p) => p.name === where.value);
      if (direction && chosen && !direction.select.dataset.touched) direction.select.value = chosen.value[2] < 0 ? 'up' : 'down';
    };
    direction?.select.addEventListener('change', () => (direction.select.dataset.touched = '1'));
    where.addEventListener('change', follow);
    follow();
    const add = el('button', { className: 'btn primary', type: 'button', text: 'Add', id: 'addPartYes' });
    const cancel = el('button', { className: 'btn', type: 'button', text: 'Cancel', id: 'addPartNo' });
    const done = (answer) => {
      host.hidden = true;
      host.replaceChildren();
      resolve(answer);
    };
    add.addEventListener('click', () => {
      const chosen = ports.find((p) => p.name === where.value);
      const values = {};
      for (const { p, select } of asks) values[p.name] = p.choices.find((c) => String(c) === select.value) ?? select.value;
      const shape = String(values.shape ?? '');
      done({ component: component.name, port: where.value, values: { ...(chosen ? shadeDefaults(shape, chosen.value) : {}), ...values } });
    });
    cancel.addEventListener('click', () => done(null));
    host.append(
      el('h2', { text: `Add ${String(component.label).toLowerCase()}` }),
      field('Mounted on', where),
      ...asks.map(({ p, select }) => field(p.label, select)),
      el('div', { className: 'buttons' }, [add, cancel]),
    );
    host.hidden = false;
    where.focus();
  });
}
