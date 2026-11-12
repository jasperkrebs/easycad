// Tests for list settings and projects with several parts, as the app and the agent see them
// (web/js/settings.js, web/js/agent.js, the Plan's add buttons in web/js/parts.js), and for
// telling when a project's template has been improved since (web/js/templates.js); run with
// `node --test scripts/` (CI does).

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runStatus } from '../web/js/agent.js';
import { parseNumber } from '../web/js/dom.js';
import { assemblyMotion, assemblySteps, freePorts, newPartName, sections, shapeSwitch, threadOf, withMounted } from '../web/js/parts.js';
import { applies, listValue } from '../web/js/settings.js';
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
    template.replace('c_disc    = link_rings(reverse(c_rings), "spiral", line)', 'c_disc    = link_rings(c_rings, "spiral", disc_w)'),
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

test('a setting with `when` applies only while the setting it names has one of its values', () => {
  const params = [
    { name: 'shape', value: 'cone' },
    { name: 'own_r', value: 50, when: { name: 'shape', values: ['own'], not: false } },
    { name: 'lip', value: false, when: { name: 'shape', values: ['sphere', 'flat sphere'], not: false } },
    { name: 'lip_length', value: 10, when: { name: 'lip', values: [], not: false } },
    { name: 'strength', value: 100, when: { name: 'comp', values: ['off'], not: true } },
    { name: 'comp', value: 'width' },
    { name: 'gap', value: 1, when: { name: 'missing', values: [], not: false } },
  ];
  const at = (values) => Object.fromEntries(params.map((p) => [p.name, applies(p, params, values)]));
  let v = at({ shape: 'cone', lip: true, comp: 'width' });
  assert.equal(v.own_r, false);
  assert.equal(v.lip, false);
  assert.equal(v.lip_length, false, 'the lip is on, but the lip itself doesn\'t apply to a cone');
  assert.equal(v.strength, true);
  assert.equal(v.gap, true, 'a condition on a setting that isn\'t there never hides anything');
  v = at({ shape: 'flat sphere', lip: true, comp: 'off' });
  assert.equal(v.lip, true);
  assert.equal(v.lip_length, true);
  assert.equal(v.strength, false);
  assert.equal(at({ shape: 'own', lip: false, comp: 'width' }).own_r, true);
});

test('stored settings never throw, with storage or without', async () => {
  const { load, save, loadJSON, saveJSON } = await import('../web/js/dom.js');
  const had = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
  try {
    // Storage that throws on every use (private browsing in some browsers): fallbacks, no errors.
    const broken = () => {
      throw new Error('SecurityError');
    };
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: { getItem: broken, setItem: broken, removeItem: broken } });
    assert.equal(load('k', 'fallback'), 'fallback');
    assert.deepEqual(loadJSON('k', { a: 1 }), { a: 1 });
    save('k', 'v');
    saveJSON('k', { a: 2 });
    // Working storage: values round-trip, null removes, unreadable JSON gives the fallback.
    const map = new Map();
    Object.defineProperty(globalThis, 'localStorage', {
      configurable: true,
      value: { getItem: (k) => map.get(k) ?? null, setItem: (k, v) => map.set(k, String(v)), removeItem: (k) => map.delete(k) },
    });
    save('k', 'v');
    assert.equal(load('k'), 'v');
    save('k', null);
    assert.equal(load('k', 'gone'), 'gone');
    saveJSON('j', { a: 3 });
    assert.deepEqual(loadJSON('j', null), { a: 3 });
    map.set('bad', '{nope');
    assert.equal(loadJSON('bad', 'fallback'), 'fallback');
  } finally {
    if (had) Object.defineProperty(globalThis, 'localStorage', had);
    else delete globalThis.localStorage;
  }
});

// ---- The Plan's add buttons (web/js/parts.js): where a new shade can go, and what it starts with.

/**
 * An engine result with parts' toolpaths: each part a straight wall at radius `r` from `z0` to
 * `z1` (a point a millimetre), and ports `[radius, height, inward, length, socket, side]`.
 */
function planResult(parts, ports) {
  const points = [];
  const starts = [0];
  const widths = [];
  for (const { r, z0, z1 } of parts) {
    for (let z = z0; z <= z1; z += 1) {
      points.push(r, 0, z);
      widths.push(6);
    }
    starts.push(points.length / 3);
  }
  return {
    paths: { points, starts, widths },
    parts: { paths: parts.map((_, i) => i) },
    prints: parts.map((p) => ({ name: p.name })),
    ports: ports.map(([part, name, value]) => ({ part, name: `${part}.${name}`, label: name, value })),
  };
}

test('a new shade goes on a free face, further along where one already sits', () => {
  // A tube (r 30, z 0..200); a shade's holder outside its top (r 36, z 180..200).
  const result = planResult(
    [
      { name: 'tube', r: 30, z0: 0, z1: 200 },
      { name: 'cone', r: 36, z0: 180, z1: 200 },
    ],
    [
      ['tube', 'end_out', [36, 200, -1, 200, 0, 1]],
      ['tube', 'end_in', [24, 200, -1, 200, 0, -1]],
      ['tube', 'start_in', [24, 0, 1, 0, 0, -1]],
      ['cone', 'end_out', [42, 300, -1, 20, 0, 1]],
    ],
  );
  const along = Object.fromEntries(freePorts(result, sections(result)).map((f) => [f.port.name, f.along]));
  assert.equal(along['tube.end_in'], 0, 'inside: free at the end');
  assert.ok(along['tube.end_out'] >= 22 && along['tube.end_out'] <= 24, `outside: past the cone's holder (${along['tube.end_out']})`);
  assert.equal('tube.start_in' in along, false, 'no section there');
  assert.equal(along['cone.end_out'], 0);
  // A short section that's taken offers nothing; a hidden part's ports aren't offered.
  const short = planResult(
    [
      { name: 'a', r: 30, z0: 0, z1: 20 },
      { name: 'b', r: 36, z0: 0, z1: 20 },
    ],
    [['a', 'start_out', [36, 0, 1, 20, 0, 1]]],
  );
  assert.deepEqual(freePorts(short, sections(short)), []);
  assert.deepEqual(
    freePorts(result, sections(result), new Set(['tube'])).map((f) => f.port.name),
    ['cone.end_out'],
  );
});

test("a brim (a print aid) isn't in the plan, so it takes no face", () => {
  // A tube (r 30, z 0..60) printed with a brim: rings at r 36 and 42 on the bed, marked as aids.
  const result = planResult([{ name: 'tube', r: 30, z0: 0, z1: 60 }], [['tube', 'start_out', [36, 0, 1, 60, 0, 1]]]);
  const n = result.paths.points.length / 3;
  for (const r of [36, 42]) {
    for (let a = 0; a < 12; a++) result.paths.points.push(r * Math.cos(a / 2), r * Math.sin(a / 2), 0.5);
  }
  result.paths.widths.push(...Array(24).fill(6));
  result.paths.starts[1] = result.paths.points.length / 3;
  result.paths.aids = new Uint8Array(result.paths.points.length / 3).fill(1, n);
  const plan = sections(result);
  assert.ok(plan.get('tube').points.every(([r]) => Math.abs(r - 30) < 1e-9), 'only the wall');
  assert.equal(freePorts(result, plan)[0].along, 0, 'the start is free from its end');
  // Without the marks the brim is drawn, and takes the face where it lies.
  delete result.paths.aids;
  assert.ok(freePorts(result, sections(result))[0].along > 0);
});

test("a shade's new shape brings its sizes for the settings it didn't use before", () => {
  const port = [30, 100, -1, 20, 0, 1];
  assert.deepEqual(shapeSwitch('cone', 'sphere', port), { sphere: 214 });
  assert.deepEqual(shapeSwitch('cone', 'tube', port), {});
  assert.deepEqual(shapeSwitch('tube', 'cone', port), { opening: 164 });
  assert.deepEqual(shapeSwitch('tube', 'cone', null), {}, "on the socket: the template's own");
  assert.equal(newPartName('bell in-out', ['bell_in_out']), 'bell_in_out2');
});

test('a shade slides along the thread it is screwed onto, the shades on it with it', () => {
  // A tube on the sphere's end (outside), a cone on the tube's end (inside, 30 mm in), and a
  // dome on the cone; the sphere on the socket.
  const param = (name, value, extra = {}) => ({ name, value, kind: 'number', ...extra });
  const result = {
    partList: ['main', 'tube', 'cone', 'dome'].map((name) => ({ name })),
    params: [
      param('joint', 0),
      param('main.mount', 'on_socket'),
      param('main.along', 0),
      param('tube.mount', 'main.end_out'),
      param('tube.along', 0),
      param('tube.start_len', 20),
      param('tube.shape', 'tube'),
      param('tube.length', 200),
      param('cone.mount', 'tube.end_in'),
      param('cone.along', 30, { max: 1000 }),
      param('cone.start_len', 25),
      param('cone.shape', 'cone'),
      param('dome.mount', 'cone.start_out'),
      param('dome.along', 0),
    ],
    ports: [
      { name: 'on_socket', part: '', value: [23, 0, 1, 100000, 1, 1] },
      { name: 'main.end_out', part: 'main', value: [81, 214, -1, 20, 0, 1] },
      { name: 'tube.end_in', part: 'tube', value: [75, 394, -1, 200, 0, -1] },
    ],
  };
  // On the socket: no slide (the socket's thread is the socket's).
  assert.equal(threadOf(result, 'main'), null);
  // The tube's holder fills the sphere's straight end: no room to slide.
  assert.deepEqual({ ...threadOf(result, 'tube'), port: undefined }, { port: undefined, inward: -1, along: 0, hold: 20, max: 0 });
  // The cone can go as far along the tube as leaves its 25 mm holder on it.
  const cone = threadOf(result, 'cone');
  assert.equal(cone.inward, -1);
  assert.equal(cone.along, 30);
  assert.equal(cone.max, 175);
  // Moving the tube moves the cone and the dome; moving the cone, the dome.
  assert.deepEqual(withMounted(result, 'tube'), ['tube', 'cone', 'dome']);
  assert.deepEqual(withMounted(result, 'cone'), ['cone', 'dome']);
  assert.deepEqual(withMounted(result, 'dome'), ['dome']);
});

test('taken apart: furthest out first, each turned off its thread then moved on by its thread', () => {
  // A tube screwed onto the sphere's end (going up, the section running down from its end), a
  // cone screwed onto the tube's end, 30 mm along; the sphere on the socket stays.
  const param = (name, value) => ({ name, value, kind: 'number' });
  const result = {
    partList: ['main', 'tube', 'cone'].map((name) => ({ name })),
    prints: ['main', 'tube', 'cone'].map((name) => ({ name })),
    params: [
      param('joint', 0),
      param('main.mount', 'on_socket'),
      param('main.along', 0),
      param('tube.mount', 'main.end_out'),
      param('tube.along', 0),
      param('tube.start_len', 20),
      param('tube.shape', 'tube'),
      param('tube.length', 200),
      param('cone.mount', 'tube.end_out'),
      param('cone.along', 30),
      param('cone.start_len', 20),
      param('cone.shape', 'cone'),
    ],
    ports: [
      { name: 'on_socket', part: '', value: [23, 0, 1, 100000, 1, 1] },
      { name: 'main.end_out', part: 'main', value: [81, 214, -1, 20, 0, 1] },
      { name: 'tube.end_out', part: 'tube', value: [87, 394, -1, 200, 0, 1] },
    ],
    // Every part's toolpath printed 2.5 mm a turn.
    paths: { starts: [0, 2, 4, 6], heights: new Float32Array([2.5, 2.5, 2.5, 2.5, 2.5, 2.5]) },
    parts: { paths: [0, 1, 2] },
  };
  const steps = assemblySteps(result);
  assert.deepEqual(steps.map((s) => s.name), ['cone', 'tube']);
  assert.deepEqual(steps[0], { name: 'cone', parent: 'tube', out: 1, travel: 50, gap: 20, pitch: 2.5 });
  // Half way: the cone is off (its 50 mm of thread in 20 turns, then 20 mm on), the tube still on.
  const half = assemblyMotion(steps, 0.5);
  assert.deepEqual(half.get('cone'), { angle: 7200, dz: 70 });
  assert.deepEqual(half.get('tube'), { angle: 0, dz: 0 });
  // A quarter: the cone half way through its 70 mm, 35 mm up, 14 turns.
  assert.deepEqual(assemblyMotion(steps, 0.25).get('cone'), { angle: 5040, dz: 35 });
  // Exploded: the tube off too (20 + 20 mm, 8 turns); the cone rode up with it without turning
  // any more (it was off already: only the shade coming off turns).
  const all = assemblyMotion(steps, 1);
  assert.deepEqual(all.get('tube'), { angle: 2880, dz: 40 });
  assert.deepEqual(all.get('cone'), { angle: 7200, dz: 110 });
  assert.deepEqual(assemblyMotion(steps, 0.75).get('cone'), { angle: 7200, dz: 90 });
  assert.equal(all.has('main'), false, 'on the socket it stays');
});
