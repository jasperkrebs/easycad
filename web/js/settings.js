// The settings window, generated from the recipe's `param` lines: groups (each a fold-out), sliders with number
// fields, toggles, dropdowns, text fields, and tables for list settings (rows of numbers, like a
// profile's points). Values are shown and edited in each setting's own unit (18 %, 90°),
// exactly as they are written in the recipe.

import { el, loadJSON, numberField, parseNumber, saveJSON } from './dom.js';

const UNIT_LABELS = { deg: '°', inch: 'in' };

/** A slider step for a range: about a hundredth of it, rounded down to a power of ten. */
function autoStep(min, max) {
  const span = Math.abs(max - min);
  return span > 0 ? 10 ** Math.floor(Math.log10(span / 100)) : 1;
}

/** Whether a list setting holds rows of numbers (`[[0, 30], [1, 28]]`) rather than numbers. */
const hasRows = (value) => Array.isArray(value) && value.some(Array.isArray);

/**
 * A list setting's new value, checked: numbers, or rows of numbers, the same kind as the
 * setting's own (rows stay rows). Text is read as JSON (`[[0, 30], [1, 28]]`).
 * @param {{ value: any }} p the setting
 * @param {unknown} raw
 * @returns {{ value: any[] } | { error: string }}
 */
export function listValue(p, raw) {
  let v = raw;
  if (typeof v === 'string') {
    try {
      v = JSON.parse(v);
    } catch {
      v = null;
    }
  }
  const rows = hasRows(p.value);
  const example = rows ? '[[0, 30], [1, 28]]' : '[10, 20, 30]';
  if (!Array.isArray(v)) return { error: `needs a list like ${example}` };
  const number = (x) => (typeof x === 'number' || (typeof x === 'string' && x.trim() !== '')) && Number.isFinite(Number(x));
  if (rows) {
    if (!v.every((r) => Array.isArray(r) && r.length > 0 && r.every(number))) return { error: `needs rows of numbers, like ${example}` };
    return { value: v.map((r) => r.map(Number)) };
  }
  if (!v.every(number)) return { error: `needs numbers, like ${example}` };
  return { value: v.map(Number) };
}

/** Everything about the settings window except the values, to know when it must be rebuilt. */
export function layoutKey(params) {
  return JSON.stringify(params.map((p) => [p.name, p.label, p.group, p.kind, p.unit, p.min, p.max, p.step, p.choices, p.attrs, p.when, p.part]));
}

/**
 * Builds the settings window.
 * @param {HTMLElement} host
 * @param {any[]} params the engine's ParamInfo list
 * @param {Record<string, any>} values current values by name
 * @param {(name: string, value: any, quality: 'preview' | 'final') => void} onChange
 * @param {{ broken?: boolean, ports?: any[], components?: any[], onAddPart?: () => void, onRemovePart?: (name: string) => void }} [opts]
 *   broken: the recipe has errors (they're explained elsewhere); ports, components: the
 *   engine's, for parts (each part its own fold-out, an input a list of ports)
 */
export function renderSettings(host, params, values, onChange, opts = {}) {
  host.replaceChildren();
  const groups = new Map();
  /** @type {Map<string, { details: HTMLElement, body: HTMLElement, group: string | null }>} */
  const parts = new Map();
  /** @type {Map<string, HTMLElement>} */
  const rows = new Map();
  // Settings that apply only while another has a value (`when`): shown or hidden after every
  // change, and their groups' counts with them.
  const refresh = () => {
    for (const p of params) {
      const row = rows.get(p.name);
      if (row) row.hidden = !applies(p, params, values);
    }
    for (const [name, f] of parts) {
      const count = f.details.querySelector('.group-count');
      if (count) count.textContent = String(f.details.querySelectorAll(':scope .setting:not([hidden])').length);
      const shape = values[`${name}.shape`];
      const tag = f.details.querySelector('.part-shape');
      if (tag) tag.textContent = shape == null ? '' : String(shape);
    }
    for (const g of groups.values()) {
      const shown = g.querySelectorAll(':scope .setting:not([hidden])').length;
      const count = g.querySelector('.group-count');
      if (count) count.textContent = String(shown);
      g.hidden = shown === 0;
    }
  };
  const change = (name, value, quality) => {
    onChange(name, value, quality);
    refresh();
  };
  for (const p of params) {
    if (p.attrs.includes('hidden')) continue;
    if (p.part) {
      let f = parts.get(p.part);
      if (!f) {
        f = partFoldout(p.part, opts.onRemovePart);
        parts.set(p.part, f);
        host.append(f.details);
      }
      if ((p.group || null) !== f.group) {
        f.group = p.group || null;
        if (f.group) f.body.append(el('div', { className: 'subgroup', text: f.group }));
      }
      const row = p.kind === 'port' ? portRow(p, values[p.name] ?? p.value, change, params, values, opts.ports || []) : settingRow(p, values[p.name] ?? p.value, change);
      rows.set(p.name, row);
      f.body.append(row);
      continue;
    }
    const name = p.group || '';
    if (!groups.has(name)) {
      groups.set(name, name ? foldout(name, ![...groups.keys()].some(Boolean)) : el('div', { className: 'group' }));
      host.append(groups.get(name));
    }
    const g = groups.get(name);
    const row = settingRow(p, values[p.name] ?? p.value, change);
    rows.set(p.name, row);
    (g.querySelector('.group-body') || g).append(row);
  }
  for (const f of parts.values()) f.body.append(f.remove);
  // A part from each component (a shade, …).
  for (const c of opts.components || []) {
    if (!opts.onAddPart) break;
    const add = el('button', { className: 'btn add-part', type: 'button', text: `Add ${String(c.label).toLowerCase()}` });
    add.addEventListener('click', () => opts.onAddPart?.());
    host.append(add);
  }
  refresh();
  if (!params.length && !opts.broken) {
    host.append(el('p', { className: 'hint', text: 'No settings.' }));
  }
}

/**
 * A part's fold-out: its name and shape as the header, its settings underneath (with its own
 * groups as headings), and a button that removes it.
 * @param {string} name
 * @param {((name: string) => void) | undefined} onRemove
 */
function partFoldout(name, onRemove) {
  const details = foldout(name, false, `part:${name}`);
  details.classList.add('part');
  const summary = details.querySelector('summary');
  summary?.querySelector('.group-name')?.after(el('span', { className: 'part-shape' }));
  const body = /** @type {HTMLElement} */ (details.querySelector('.group-body'));
  const remove = el('button', { className: 'btn remove-part', type: 'button', text: `Remove ${name}` });
  remove.addEventListener('click', () => onRemove?.(name));
  return { details, body, group: null, remove };
}

/**
 * Where a part is mounted: a list of the ports it can go on (not its own, nor those of parts
 * mounted on it), plus what it reads by default (the socket). Ports with no length (a shade
 * with no straight end) aren't offered.
 */
function portRow(p, value, onChange, params, values, ports) {
  const id = `set-${p.name}`;
  const row = el('div', { className: 'setting choice' }, [el('label', { htmlFor: id, text: p.label })]);
  const parentOf = (part) => {
    const v = values[`${part}.mount`] ?? params.find((q) => q.name === `${part}.mount`)?.value;
    return typeof v === 'string' && v.includes('.') ? v.split('.')[0] : null;
  };
  // This part and everything mounted on it, directly or further out.
  const own = new Set([p.part]);
  for (let grew = true; grew; ) {
    grew = false;
    for (const q of params) {
      if (!q.part || own.has(q.part)) continue;
      const parent = parentOf(q.part);
      if (parent && own.has(parent)) {
        own.add(q.part);
        grew = true;
      }
    }
  }
  const options = [];
  for (const c of p.choices) options.push([String(c), humanize(String(c))]);
  for (const port of ports) {
    if (own.has(port.part)) continue;
    if (!(port.value[3] > 0) && port.name !== value) continue;
    options.push([port.name, `${port.part} · ${port.label}`]);
  }
  if (value && !options.some(([v]) => v === value)) options.push([String(value), String(value)]);
  const select = /** @type {HTMLSelectElement} */ (el('select', { id }, options.map(([v, t]) => el('option', { value: v, text: t }))));
  select.value = String(value);
  select.addEventListener('change', () => onChange(p.name, select.value, 'final'));
  row.append(select);
  return row;
}

/** `on_socket` → "On socket". */
const humanize = (name) => {
  const s = name.replace(/_/g, ' ');
  return s.charAt(0).toUpperCase() + s.slice(1);
};

/** Whether a value counts as on: true, a number other than 0, or anything not empty. */
const on = (v) => (Array.isArray(v) ? v.length > 0 : typeof v === 'number' ? v !== 0 : Boolean(v));

/**
 * Whether a setting applies now: its `when` holds (the setting it names has one of its values,
 * or is on), and that setting applies itself.
 * @param {any} p the engine's ParamInfo
 * @param {any[]} params
 * @param {Record<string, any>} values
 * @param {number} [depth]
 */
export function applies(p, params, values, depth = 0) {
  const w = p.when;
  if (!w || depth > 20) return true;
  const other = params.find((q) => q.name === w.name);
  if (!other) return true;
  const v = values[w.name] ?? other.value;
  const holds = w.values.length ? w.values.some((x) => String(x) === String(v)) : on(v);
  return (w.not ? !holds : holds) && applies(other, params, values, depth + 1);
}

/** Which settings groups are open, by name (remembered on this device). */
const OPEN_KEY = 'easycad.openGroups';

/** @returns {Record<string, boolean>} */
function openGroups() {
  const v = loadJSON(OPEN_KEY, {});
  return v && typeof v === 'object' ? v : {};
}

/**
 * One settings group as a fold-out: its name (and how many settings it holds) as the header,
 * the settings underneath. The first group starts open, the others closed, until you open or
 * close them; that's remembered by name, so every project with a "Print" group opens it alike.
 * @param {string} name
 * @param {boolean} first
 * @param {string} [key] what its open state is remembered under (its name)
 */
function foldout(name, first, key = name) {
  const remembered = openGroups()[key];
  const details = /** @type {HTMLDetailsElement} */ (
    el('details', { className: 'group' }, [
      el('summary', {}, [el('span', { className: 'group-name', text: name }), el('span', { className: 'group-count' })]),
      el('div', { className: 'group-body' }),
    ])
  );
  details.open = remembered ?? first;
  details.addEventListener('toggle', () => {
    saveJSON(OPEN_KEY, { ...openGroups(), [key]: details.open });
  });
  return details;
}

function settingRow(p, value, onChange) {
  const id = `set-${p.name}`;
  const label = el('label', { htmlFor: id, text: p.label });
  if (p.attrs.includes('assumed')) {
    label.append(' ', el('span', { className: 'badge', text: 'assumed', title: 'A guessed value: check it.' }));
  }
  const row = el('div', { className: `setting ${p.kind}` }, [label]);

  if (p.kind === 'toggle') {
    const box = /** @type {HTMLInputElement} */ (el('input', { type: 'checkbox', id, checked: value === true, className: 'switch' }));
    box.addEventListener('change', () => onChange(p.name, box.checked, 'final'));
    row.append(box);
    return row;
  }
  if (p.kind === 'choice') {
    const select = /** @type {HTMLSelectElement} */ (
      el('select', { id }, p.choices.map((c) => el('option', { value: String(c), text: String(c) })))
    );
    select.value = String(value);
    select.addEventListener('change', () => {
      const choice = p.choices.find((c) => String(c) === select.value);
      onChange(p.name, choice ?? select.value, 'final');
    });
    row.append(select);
    return row;
  }
  if (p.kind === 'text') {
    const input = /** @type {HTMLInputElement} */ (el('input', { type: 'text', id, value: String(value) }));
    input.addEventListener('change', () => onChange(p.name, input.value, 'final'));
    row.append(input);
    return row;
  }
  if (p.kind === 'list') {
    row.append(listTable(p, value, id, onChange));
    return row;
  }

  // A number: a number field, plus a slider when the recipe gives a range.
  const ranged = p.min != null && p.max != null;
  const step = p.step ?? (ranged ? autoStep(p.min, p.max) : null);
  let current = Number(value);
  const field = numberField({ id, value: String(current) }, !ranged || p.min < 0);
  const num = field.input;
  row.append(el('span', { className: 'num' }, [...field.nodes, el('span', { className: 'unit', text: UNIT_LABELS[p.unit] ?? p.unit })]));

  /** Keeps a typed value inside the range, and on the recipe's own step if it has one. */
  const tidy = (v) => {
    let out = ranged ? Math.min(p.max, Math.max(p.min, v)) : v;
    if (p.step) {
      const base = ranged ? p.min : 0;
      out = base + Math.round((out - base) / p.step) * p.step;
      out = Number(out.toFixed(10));
    }
    return out;
  };

  /** @type {HTMLInputElement | null} */
  let range = null;
  if (ranged) {
    range = /** @type {HTMLInputElement} */ (
      el('input', { type: 'range', min: String(p.min), max: String(p.max), step: String(step), value: String(current), 'aria-label': p.label })
    );
    const slider = range;
    // Dragging asks for quick previews; letting go asks for the full-quality result.
    slider.addEventListener('input', () => {
      current = Number(slider.value);
      num.value = slider.value;
      onChange(p.name, current, 'preview');
    });
    slider.addEventListener('change', () => {
      current = Number(slider.value);
      num.value = slider.value;
      onChange(p.name, current, 'final');
    });
    row.append(slider);
  }
  num.addEventListener('change', () => {
    const v = parseNumber(num.value);
    if (num.value.trim() === '' || !Number.isFinite(v)) {
      num.value = String(current);
      return;
    }
    current = tidy(v);
    num.value = String(current);
    if (range) range.value = String(current);
    onChange(p.name, current, 'final');
  });
  return row;
}

/**
 * A list setting as a table: a row of number fields per item (one field for a list of numbers),
 * a button to remove each row and one to add a row (a copy of the last). Every change is the
 * whole list, written back into the recipe.
 */
function listTable(p, value, id, onChange) {
  const rows = hasRows(p.value) || hasRows(value);
  /** @type {number[][]} */
  let items = (Array.isArray(value) ? value : []).map((r) => (Array.isArray(r) ? r.map(Number) : [Number(r)]));
  const width = Math.max(1, ...items.map((r) => r.length), ...(Array.isArray(p.value) ? p.value.map((r) => (Array.isArray(r) ? r.length : 1)) : []));
  const table = el('div', { className: 'list-table' });
  const commit = () => onChange(p.name, rows ? items.map((r) => [...r]) : items.map((r) => r[0]), 'final');
  const draw = () => {
    table.replaceChildren();
    items.forEach((item, i) => {
      const fields = item.map((v, j) => {
        const { input } = numberField({
          value: String(v),
          id: i === 0 && j === 0 ? id : `${id}-${i}-${j}`,
          'aria-label': `${p.label}: row ${i + 1}${width > 1 ? `, value ${j + 1}` : ''}`,
        });
        input.addEventListener('change', () => {
          const n = parseNumber(input.value);
          if (input.value.trim() === '' || !Number.isFinite(n)) {
            input.value = String(items[i][j]);
            return;
          }
          items[i][j] = n;
          commit();
        });
        return input;
      });
      const remove = el('button', { className: 'btn tiny', type: 'button', text: '−', title: 'Remove this row', 'aria-label': `Remove row ${i + 1} of ${p.label}` });
      remove.addEventListener('click', () => {
        items.splice(i, 1);
        draw();
        commit();
      });
      table.append(el('div', { className: 'list-row' }, [...fields, remove]));
    });
    const add = el('button', { className: 'btn tiny', type: 'button', text: '+ Add a row', 'aria-label': `Add a row to ${p.label}` });
    add.addEventListener('click', () => {
      items.push(items.length ? [...items[items.length - 1]] : new Array(width).fill(0));
      draw();
      commit();
    });
    table.append(el('div', { className: 'list-add' }, [add]));
  };
  draw();
  return table;
}
