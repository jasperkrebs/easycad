// The settings window, generated from the recipe's `param` lines: groups, sliders with number
// fields, toggles, dropdowns, text fields, and tables for list settings (rows of numbers, like a
// profile's points). Values are shown and edited in each setting's own unit (18 %, 90°),
// exactly as they are written in the recipe.

import { el } from './dom.js';

export const UNIT_LABELS = { deg: '°', inch: 'in' };

/** A slider step for a range: about a hundredth of it, rounded down to a power of ten. */
export function autoStep(min, max) {
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
  return JSON.stringify(params.map((p) => [p.name, p.label, p.group, p.kind, p.unit, p.min, p.max, p.step, p.choices, p.attrs]));
}

/**
 * Builds the settings window.
 * @param {HTMLElement} host
 * @param {any[]} params the engine's ParamInfo list
 * @param {Record<string, any>} values current values by name
 * @param {(name: string, value: any, quality: 'preview' | 'final') => void} onChange
 * @param {{ broken?: boolean }} [opts] broken: the recipe has errors (they're explained elsewhere)
 */
export function renderSettings(host, params, values, onChange, opts = {}) {
  host.replaceChildren();
  const groups = new Map();
  for (const p of params) {
    if (p.attrs.includes('hidden')) continue;
    const name = p.group || '';
    if (!groups.has(name)) {
      const g = el('div', { className: 'group' }, [name ? el('h2', { text: name }) : null]);
      groups.set(name, g);
      host.append(g);
    }
    groups.get(name).append(settingRow(p, values[p.name] ?? p.value, onChange));
  }
  if (!params.length && !opts.broken) {
    host.append(
      el('p', {
        className: 'hint',
        text: 'This recipe has no settings yet. Add one in the Recipe tab, for example: param height = 100 mm in 20..300 "Height"',
      }),
    );
  }
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
  const num = /** @type {HTMLInputElement} */ (el('input', { type: 'number', id, inputMode: 'decimal', step: 'any', value: String(current) }));
  row.append(el('span', { className: 'num' }, [num, el('span', { className: 'unit', text: UNIT_LABELS[p.unit] ?? p.unit })]));

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
    const v = Number(num.value);
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
        const input = /** @type {HTMLInputElement} */ (
          el('input', {
            type: 'number',
            inputMode: 'decimal',
            step: 'any',
            value: String(v),
            id: i === 0 && j === 0 ? id : `${id}-${i}-${j}`,
            'aria-label': `${p.label}: row ${i + 1}${width > 1 ? `, value ${j + 1}` : ''}`,
          })
        );
        input.addEventListener('change', () => {
          const n = Number(input.value);
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
