// The chat's text, built as DOM (no innerHTML): a little Markdown, one-line descriptions of the
// agent's tool calls, its thinking (folded), and money.

import { el } from './dom.js';
import { PRINTERS } from './print.js';

/** Dollars, with more decimals for small amounts. */
export const money = (usd) => (usd < 0.01 ? `$${usd.toFixed(4)}` : `$${usd.toFixed(usd < 1 ? 3 : 2)}`);

/** Plain text with a little Markdown (paragraphs, lists, `code`, **bold**, code blocks), built as DOM: no innerHTML. */
export function richText(text) {
  const box = el('div', { className: 'md' });
  const parts = text.split(/```[^\n]*\n?/);
  parts.forEach((part, i) => {
    if (i % 2 === 1) {
      box.append(el('pre', {}, [el('code', { text: part.replace(/\n$/, '') })]));
      return;
    }
    for (const para of part.split(/\n{2,}/)) {
      const lines = para.split('\n').filter((l) => l.trim());
      if (!lines.length) continue;
      if (lines.every((l) => /^\s*([-*]|\d+\.)\s/.test(l))) {
        box.append(el('ul', {}, lines.map((l) => el('li', {}, inline(l.replace(/^\s*([-*]|\d+\.)\s/, ''))))));
      } else {
        const p = el('p');
        lines.forEach((l, k) => {
          if (k) p.append(el('br'));
          p.append(...inline(l));
        });
        box.append(p);
      }
    }
  });
  return box;
}

/** `code` and **bold** within a line. */
function inline(text) {
  return text.split(/(`[^`]+`|\*\*[^*]+\*\*)/).filter(Boolean).map((t) => {
    if (t.startsWith('`') && t.endsWith('`')) return el('code', { text: t.slice(1, -1) });
    if (t.startsWith('**') && t.endsWith('**')) return el('b', { text: t.slice(2, -2) });
    return document.createTextNode(t);
  });
}

/** A one-line description of a tool call, for the chat. */
export function describeTool(use) {
  const i = use.input || {};
  switch (use.name) {
    case 'edit_recipe':
      return `Edited the recipe (${(i.edits || []).length} change${(i.edits || []).length === 1 ? '' : 's'})`;
    case 'write_recipe':
      return `Wrote the recipe (${String(i.recipe || '').split('\n').length} lines)`;
    case 'set_settings':
      return `Set ${Object.entries(i.values || {}).map(([k, v]) => `${k} = ${v}`).join(', ')}`;
    case 'set_printer': {
      const parts = [];
      if (i.printer) parts.push(`Switched to the ${PRINTERS[i.printer]?.name || i.printer}`);
      if (i.material) parts.push(`${parts.length ? 'used' : 'Used'} the ${i.material} profile`);
      const set = Object.entries(i.settings || {}).map(([k, v]) => `${k} = ${v}`);
      if (set.length) parts.push(`${parts.length ? 'set' : 'Set the printer’s'} ${set.join(', ')}`);
      return parts.join('; ');
    }
    case 'look':
      return `Looked at the ${i.show === 'toolpath' ? 'toolpath' : i.show === 'shape' ? 'shape' : 'model'}`;
    case 'read_recipe':
      return 'Read the recipe';
    case 'request_feature':
      return `Proposed a new step: ${i.title || ''}`;
    default:
      return use.name;
  }
}

/** The model's thinking, folded. */
export function thinkingBox(text) {
  const d = el('details', { className: 'thinking' }, [el('summary', { text: 'Thinking' }), el('p', { text })]);
  return d;
}
