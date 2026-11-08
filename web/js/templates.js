// Telling when a project's template has been improved since the project was started from it
// (app.js offers to update it, keeping the project's setting values), and the note a saved
// project file starts with, for whoever (or whichever AI) gets the file.

/**
 * A recipe with its settings' values left out (and spacing evened out): what a template and a
 * project made from it still have in common after the sliders have moved.
 * @param {string} text
 */
export function withoutValues(text) {
  return withoutParts(text)
    .split('\n')
    .map((l) => l.replace(/^(\s*param\s+[A-Za-z_]\w*\s*=)\s*(?:"(?:[^"\\]|\\.)*"(?=\s*"))?[^"\n]*/, '$1'))
    .join('\n')
    .replace(/[ \t]+/g, ' ')
    .replace(/ +$/gm, '')
    .trim();
}

/**
 * The recipe's `part` statements (`part tube = shade(…)`, over as many lines as its brackets
 * take), in order: a project's own shades, like its setting values.
 * @param {string} text
 * @returns {string[]}
 */
export function partStatements(text) {
  const out = [];
  const re = /^part\s/gm;
  let m;
  while ((m = re.exec(text))) {
    let depth = 0;
    let i = m.index;
    for (; i < text.length; i++) {
      const c = text[i];
      if (c === '"') {
        for (i++; i < text.length && text[i] !== '"'; i++) if (text[i] === '\\') i++;
      } else if (c === '(' || c === '[') depth++;
      else if (c === ')' || c === ']') depth--;
      else if (c === '\n' && depth <= 0) break;
    }
    out.push(text.slice(m.index, i));
    re.lastIndex = i;
  }
  return out;
}

/**
 * The recipe without its `part` statements (and the blank lines they leave).
 * @param {string} text
 */
export function withoutParts(text) {
  let out = text;
  for (const p of partStatements(text)) out = out.replace(p, '');
  return out.replace(/\n{3,}/g, '\n\n');
}

/**
 * A template's new version with a project's own parts in place of the template's.
 * @param {string} template
 * @param {string} project
 */
export function withPartsOf(template, project) {
  const mine = partStatements(project);
  if (!partStatements(template).length || !mine.length) return template;
  return `${withoutParts(template).trimEnd()}\n\n${mine.join('\n')}\n`;
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

/** The note's lines start with this; opening a file leaves them out again. */
const NOTE = '#|';

/**
 * The note a saved project file starts with: what the file is and how to change it with an AI.
 * @param {string} site where EasyCAD runs
 */
export function fileNote(site) {
  return [
    `EasyCAD project (${site}). This file is the whole project: the recipe, with each setting's`,
    'value in its `param` line.',
    'For an AI: change the recipe text and give back the whole file. A `param` line is a slider in',
    'the settings window; `group` lines sort them; comments start with #. For the language and',
    'every step with its inputs, paste in the text from "Copy for AI" on EasyCAD\'s Recipe tab.',
    'To use it: EasyCAD, Settings, Open. (Lines starting with #| are this note; EasyCAD leaves them',
    'out when it opens the file.)',
  ]
    .map((l) => `${NOTE} ${l}`)
    .join('\n');
}

/**
 * A project file with its note (and the blank line after it) left out.
 * @param {string} text
 */
export function withoutFileNote(text) {
  const lines = text.split('\n');
  let i = 0;
  while (i < lines.length && lines[i].startsWith(NOTE)) i++;
  if (i === 0) return text;
  while (i < lines.length && !lines[i].trim()) i++;
  return lines.slice(i).join('\n');
}
