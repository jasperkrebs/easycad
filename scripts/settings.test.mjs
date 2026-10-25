// Tests for list settings and projects with several parts, as the app and the agent see them
// (web/js/settings.js, web/js/agent.js), and for telling when a project's template has been
// improved since (web/js/templates.js); run with `node --test scripts/` (CI does).

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runStatus } from '../web/js/agent.js';
import { parseNumber } from '../web/js/dom.js';
import { listValue } from '../web/js/settings.js';
import { fileNote, fingerprint, projectOf, withoutFileNote, withoutValues } from '../web/js/templates.js';

const rows = { value: [[0, 33], [1, 32]] };
const numbers = { value: [10, 20] };

test('a list setting takes a list of its own kind', () => {
  assert.deepEqual(listValue(rows, [[0, 30], [0.5, '36'], [1, 28]]), { value: [[0, 30], [0.5, 36], [1, 28]] });
  assert.deepEqual(listValue(rows, '[[0, 30], [1, 28]]'), { value: [[0, 30], [1, 28]] }, 'typed as text');
  assert.deepEqual(listValue(rows, []), { value: [] });
  assert.deepEqual(listValue(numbers, [1, '2.5']), { value: [1, 2.5] });
  for (const bad of [[1, 2], [[0, 'x']], [[]], 'nope', 5, null]) {
    assert.match(listValue(rows, bad).error, /rows of numbers|a list like/, JSON.stringify(bad));
  }
  assert.match(listValue(numbers, [[1, 2]]).error, /needs numbers/);
  assert.match(listValue(numbers, [1, Infinity]).error, /needs numbers/);
});

test('the agent sees list settings as lists, and every part a recipe prints', () => {
  const stats = (layers, x) => ({
    layers,
    printedHeightMm: 80,
    pathLengthMm: 31000,
    volumeMm3: 35000,
    massGPla: 43,
    timeS: 1750,
    byFlow: true,
    flowMm3S: 20,
    speedMmS: 17.8,
    widthMm: 2,
    layerMm: 0.6,
    samplesPerTurn: 0,
    bounds: { min: [x - 38, -38, 0.6], max: [x + 38, 38, 80] },
  });
  const text = runStatus({
    diagnostics: [],
    params: [{ name: 'body', kind: 'list', value: [[0, 33], [1, 32]], unit: '' }],
    prints: [
      { name: 'base', stats: stats(144, 0) },
      { name: 'lid', stats: stats(50, 87) },
    ],
    exports: [],
  });
  assert.match(text, /Settings: body = \[\[0,33\],\[1,32\]\]/);
  assert.match(text, /Prints part "base": 144 layers/);
  assert.match(text, /Prints part "lid": 50 layers.*x 49\.\.125/);
  // One part reads as before.
  const one = runStatus({ diagnostics: [], params: [], print: { name: 'p', stats: stats(10, 0) }, exports: [] });
  assert.match(one, /^Prints: 10 layers/m);
  assert.match(runStatus({ diagnostics: [], params: [], exports: [] }), /Prints: nothing/);
});

test('a project made from a template matches it whatever its sliders say', () => {
  const template = readFileSync(new URL('../web/templates/salt-shaker.easycad', import.meta.url), 'utf8');
  assert.equal(projectOf(template), 'Salt & pepper shaker');
  // Values moved, as the sliders write them: numbers, lists, toggles, text.
  const moved = template
    .replace(/^(param layer\s+=) 0\.6 mm/m, '$1 0.85 mm')
    .replace(/^(param body\s+=) \[\[0, 33\], \[0\.45, 38\], \[1, 32\]\]/m, '$1 [[0, 30], [1, 28]]');
  assert.notEqual(moved, template);
  assert.equal(withoutValues(moved), withoutValues(template));
  const texts = 'param name = "Ann" "Name"\nparam on = true "On"';
  assert.equal(withoutValues(texts), withoutValues('param name  = "Bo, Cy" "Name"\nparam on = false   "On"'));
  // Anything else is a different recipe: a changed step, a new setting, a different label.
  for (const changed of [
    template.replace('c_disc    = link_rings(reverse(c_rings), "spiral", disc_w)', 'c_disc    = link_rings(c_rings, "spiral", disc_w)'),
    template.replace('group "Print"', 'group "Print"\nparam extra = 1 "Extra"'),
    template.replace('"Layer height"', '"Layer"'),
  ]) {
    assert.notEqual(withoutValues(changed), withoutValues(template));
  }
  assert.equal(fingerprint(withoutValues(moved)), fingerprint(withoutValues(template)));
  assert.notEqual(fingerprint('a'), fingerprint('b'));
  assert.equal(projectOf('easycad 0.1\nx = 1'), '');
});

test("a saved project starts with a note for whoever gets it, which opening leaves out", () => {
  const recipe = readFileSync(new URL('../web/templates/lampshade.easycad', import.meta.url), 'utf8');
  const note = fileNote('https://example.github.io/easycad/');
  assert.ok(note.split('\n').every((l) => l.startsWith('#| ')), 'every line of the note is a comment');
  assert.match(note, /Copy for AI/);
  assert.match(note, /https:\/\/example\.github\.io\/easycad\//);
  assert.equal(withoutFileNote(`${note}\n\n${recipe}`), recipe);
  // An AI that edits the file may drop the blank line or the note: both open the same.
  assert.equal(withoutFileNote(`${note}\n${recipe}`), recipe);
  assert.equal(withoutFileNote(recipe), recipe);
  // Only the note at the top: a `#|` further down is the recipe's own comment.
  assert.equal(withoutFileNote('x = 1\n#| mine'), 'x = 1\n#| mine');
});

test('a number can be typed with a decimal comma, as an iPhone in some languages does', () => {
  assert.equal(parseNumber('1.5'), 1.5);
  assert.equal(parseNumber('1,5'), 1.5);
  assert.equal(parseNumber(' 0,25 '), 0.25);
  assert.equal(parseNumber('-0,3'), -0.3);
  assert.equal(parseNumber('\u22122.5'), -2.5, 'a typographic minus');
  assert.equal(parseNumber('1,000.5'), 1000.5, 'a thousands comma before a point');
  assert.equal(parseNumber('.5'), 0.5);
  assert.equal(parseNumber('2e3'), 2000);
  for (const bad of ['', '  ', 'abc', '1,2,3', '1..2', '--1', null, undefined, '1,5mm']) {
    assert.ok(Number.isNaN(parseNumber(bad)), String(bad));
  }
});
