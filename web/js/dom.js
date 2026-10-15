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

/** "Twisted star vase" → "twisted-star-vase". */
export const slug = (name) =>
  String(name || 'project')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '') || 'project';
