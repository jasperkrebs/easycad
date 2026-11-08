// Small DOM helpers. Text from a recipe (labels, names, messages) only ever goes into the page
// through textContent, never innerHTML: a recipe can come from anywhere.

/** @param {string} id */
export const $ = (id) => /** @type {HTMLElement} */ (document.getElementById(id));

/**
 * Creates an element. `text` sets textContent; names with a dash (`aria-label`, `data-x`) are
 * set as attributes; everything else as properties (`className`, `htmlFor`, `value`, …).
 * @param {string} tag
 * @param {Record<string, any>} [props]
 * @param {(Node | string | null | undefined | false)[]} [children]
 */
export function el(tag, props = {}, children = []) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(props)) {
    if (value === undefined || value === null) continue;
    if (key === 'text') node.textContent = String(value);
    else if (key.includes('-')) node.setAttribute(key, String(value));
    else /** @type {any} */ (node)[key] = value;
  }
  for (const child of children) if (child) node.append(child);
  return node;
}

/**
 * A number typed by a person: a decimal comma works as well as a point (an iPhone in a language
 * that writes decimals with a comma shows a comma on its number keypad), and a typographic minus
 * as a hyphen. NaN when it isn't a number.
 * @param {unknown} text
 */
export function parseNumber(text) {
  let t = String(text ?? '')
    .trim()
    .replace(/\u2212/g, '-')
    .replace(/\s/g, '');
  if (t === '') return NaN;
  if (t.includes(',')) t = t.includes('.') ? t.replace(/,/g, '') : t.replace(',', '.');
  return /^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i.test(t) ? Number(t) : NaN;
}

/**
 * A field for a number: text with the decimal keypad (a `type="number"` field refuses the comma
 * on some phones), read with `parseNumber`. The phone's decimal keypad has no minus key, so a
 * field that can go negative gets a ± button beside it.
 * @param {Record<string, any>} props the input's properties
 * @param {boolean} [signed] whether the value can be negative
 * @returns {{ input: HTMLInputElement, nodes: HTMLElement[] }} the input, and what to put in the page
 */
export function numberField(props, signed = false) {
  const input = /** @type {HTMLInputElement} */ (el('input', { type: 'text', inputMode: 'decimal', autocomplete: 'off', spellcheck: false, ...props }));
  if (!signed) return { input, nodes: [input] };
  const flip = el('button', { className: 'btn tiny sign', type: 'button', text: '±', title: 'Change the sign', 'aria-label': 'Change the sign' });
  flip.addEventListener('click', () => {
    const v = parseNumber(input.value);
    if (!Number.isFinite(v)) return;
    input.value = String(-v);
    input.dispatchEvent(new Event('change', { bubbles: true }));
  });
  return { input, nodes: [flip, input] };
}

/** Offers text as a file download (on iPhone: "Save to Files"). */
export function download(name, text, type = 'text/plain') {
  const a = /** @type {HTMLAnchorElement} */ (el('a', { href: URL.createObjectURL(new Blob([text], { type })), download: name }));
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
}

/** Copies text to the clipboard. Resolves to false if the browser refused. */
export async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    // Older browsers, or no clipboard permission: the selection-and-copy fallback.
  }
  const area = /** @type {HTMLTextAreaElement} */ (el('textarea', { value: text, readOnly: true }));
  area.style.position = 'fixed';
  area.style.opacity = '0';
  document.body.append(area);
  area.select();
  let ok = false;
  try {
    ok = document.execCommand('copy');
  } catch {
    ok = false;
  }
  area.remove();
  return ok;
}

/** A number for people: grouped thousands, fixed decimals. */
export const fmt = (n, digits = 0) =>
  Number(n).toLocaleString(undefined, { minimumFractionDigits: digits, maximumFractionDigits: digits });

/** A number for people with up to `digits` decimals, as typed (2.5, not 2.50). */
export const fmtUpTo = (n, digits = 2) => Number(n).toLocaleString(undefined, { maximumFractionDigits: digits });

/** "Twisted star vase" → "twisted-star-vase". */
export const slug = (name) =>
  String(name || 'project')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '') || 'project';

// Browser storage, which can be unavailable (private browsing, blocked site data, or a browser
// that throws on access): reads then give the fallback and writes are dropped, so a setting
// lasts until the page closes or isn't remembered at all. Never throws.

/** @param {boolean} session */
const storageArea = (session) => (session ? globalThis.sessionStorage : globalThis.localStorage);

/**
 * A stored string, or `fallback` when there's none (or no storage).
 * @param {string} key
 * @param {string} [fallback]
 * @param {{ session?: boolean }} [where] sessionStorage instead of localStorage
 */
export function load(key, fallback = '', { session = false } = {}) {
  try {
    const v = storageArea(session).getItem(key);
    return v === null ? fallback : v;
  } catch {
    return fallback;
  }
}

/**
 * Stores a string; `null` removes the key.
 * @param {string} key
 * @param {string | null} value
 * @param {{ session?: boolean }} [where]
 */
export function save(key, value, { session = false } = {}) {
  try {
    if (value === null) storageArea(session).removeItem(key);
    else storageArea(session).setItem(key, value);
  } catch {
    // No storage: not remembered.
  }
}

/**
 * A stored JSON value, or `fallback` when there's none, it doesn't parse, or it's null.
 * @template T
 * @param {string} key
 * @param {T} fallback
 * @returns {T | any}
 */
export function loadJSON(key, fallback) {
  try {
    return JSON.parse(load(key, 'null')) ?? fallback;
  } catch {
    return fallback;
  }
}

/** @param {string} key @param {any} value */
export const saveJSON = (key, value) => save(key, JSON.stringify(value));
