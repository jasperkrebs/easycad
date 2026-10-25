// Telling when a project's template has been improved since the project was started from it
// (app.js offers to update it, keeping the project's setting values), and the note a saved
// project file starts with, for whoever (or whichever AI) gets the file.

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
