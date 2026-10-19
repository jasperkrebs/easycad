// Telling when a project's template has been improved since the project was started from it
// (app.js offers to update it, keeping the project's setting values).

/**
 * A recipe with its settings' values left out (and spacing evened out): what a template and a
 * project made from it still have in common after the sliders have moved.
 * @param {string} text
 */
export function withoutValues(text) {
  return text
    .split('\n')
    .map((l) => l.replace(/^(\s*param\s+[A-Za-z_]\w*\s*=)\s*(?:"(?:[^"\\]|\\.)*"(?=\s*"))?[^"\n]*/, '$1'))
    .join('\n')
    .replace(/[ \t]+/g, ' ')
    .replace(/ +$/gm, '')
    .trim();
}

/**
 * The project name a recipe gives itself (its `project "…"` line).
 * @param {string} text
 */
export function projectOf(text) {
  return (text.match(/^project\s+"((?:[^"\\]|\\.)*)"/m) || [])[1] || '';
}

/**
 * A short fingerprint of a text, to remember which version of a template was turned down.
 * @param {string} text
 */
export function fingerprint(text) {
  let h = 5381;
  for (let i = 0; i < text.length; i++) h = ((h * 33) ^ text.charCodeAt(i)) >>> 0;
  return h.toString(36);
}
