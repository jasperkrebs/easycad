// The Recipe tab: the project file itself. Problems are listed by line (tap one to jump there),
// every named step shows what it made, and "Copy for AI" packs the recipe with a description of
// the language, so any Claude chat can write or change recipes until the agent lives in the app.

import { el } from './dom.js';
import { LANGUAGE_GUIDE } from './language-guide.js';

/**
 * @param {HTMLElement} host
 * @param {{ line: number, col: number, severity: string, message: string }[]} diagnostics
 * @param {(line: number) => void} onPick
 */
export function renderProblems(host, diagnostics, onPick) {
  host.replaceChildren(
    ...diagnostics.map((d) => {
      const item = el('li', { 'data-severity': d.severity }, [
        el('button', { type: 'button', className: 'linkish', text: `Line ${d.line}` }),
        el('span', { text: d.message }),
      ]);
      item.querySelector('button')?.addEventListener('click', () => onPick(d.line));
      return item;
    }),
  );
  host.hidden = diagnostics.length === 0;
}

/**
 * @param {HTMLElement} host
 * @param {{ name: string, line: number, status: string, summary: string }[]} steps
 */
export function renderSteps(host, steps) {
  host.replaceChildren(
    ...steps.map((s) =>
      el('li', {}, [
        el('span', { className: `dot ${s.status === 'ok' ? '' : s.status === 'skipped' ? 'warn' : 'bad'}` }),
        el('code', { text: s.name }),
        el('span', { className: 'muted', text: s.summary }),
      ]),
    ),
  );
}

/** Puts the cursor on a line of the recipe editor and scrolls it into view. */
export function selectLine(area, line) {
  const lines = area.value.split('\n');
  const start = lines.slice(0, line - 1).reduce((n, l) => n + l.length + 1, 0);
  const end = start + (lines[line - 1] || '').length;
  area.focus();
  area.setSelectionRange(start, end);
  const lineHeight = parseFloat(getComputedStyle(area).lineHeight) || 18;
  area.scrollTop = Math.max(0, (line - 4) * lineHeight);
}

/** A prompt for any Claude chat: how the language works, every step, and this recipe. */
export function aiPrompt(source, catalog) {
  return `I'm designing a 3D-printing project in EasyCAD. A project is a recipe written in a small text language ("easycad 0.1"). Please write or change the recipe below.

${LANGUAGE_GUIDE}

The steps you can use:

${catalog.trim()}

My current recipe:

\`\`\`
${source.trim()}
\`\`\`

What I want: (describe the shape or the change here)

Reply with the complete recipe in one code block. Keep the values I'll want to tune as \`param\` lines with sensible ranges and labels. If a step I'd need doesn't exist, say so instead of guessing.
`;
}
