// Tests for list settings and projects with several parts, as the app and the agent see them
// (web/js/settings.js, web/js/agent.js), run with `node --test scripts/` (CI does).

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { runStatus } from '../web/js/agent.js';
import { listValue } from '../web/js/settings.js';

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
