// The settings window, generated from the recipe's `param` lines: groups, sliders with number
// fields, toggles, dropdowns and text fields. Values are shown and edited in each setting's own
// unit (18 %, 90°), exactly as they are written in the recipe.

import { el } from './dom.js';

export const UNIT_LABELS = { deg: '°', inch: 'in' };

/** A slider step for a range: about a hundredth of it, rounded down to a power of ten. */
export function autoStep(min, max) {
  const span = Math.abs(max - min);
  return span > 0 ? 10 ** Math.floor(Math.log10(span / 100)) : 1;
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
