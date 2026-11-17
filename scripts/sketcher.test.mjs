// Tests for reading and writing the Thread Lampshade Sketcher's settings files
// (web/js/sketcher.js), against its own files (fixtures/sketcher/); run with `node --test scripts/`.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { isSketcherSettings, sketcherToValues, valuesToSketcher } from '../web/js/sketcher.js';

const fixture = (name) => JSON.parse(readFileSync(new URL(`../fixtures/sketcher/${name}.json`, import.meta.url), 'utf8'));
const template = readFileSync(new URL('../web/templates/thread-pendant.easycad', import.meta.url), 'utf8');
const params = [...template.matchAll(/^param\s+(\w+)\s*=/gm)].map((m) => m[1]);

test("the sketcher's files are told from EasyCAD's own settings", () => {
  for (const name of ['straight', 'sphere', 'angled', 'halfsphere']) assert.ok(isSketcherSettings(fixture(name)), name);
  assert.ok(!isSketcherSettings({ app: 'EasyCAD', kind: 'settings', values: { height: 200 } }));
  assert.ok(!isSketcherSettings(null));
});

test('every value is a setting of the Thread pendant', () => {
  for (const name of ['straight', 'sphere', 'angled', 'halfsphere']) {
    const { values } = sketcherToValues(fixture(name));
    for (const k of Object.keys(values)) assert.ok(params.includes(k), `${name}: ${k}`);
  }
});

test('the angled shade: its corner, its cone, five entries on an E27', () => {
  const { values, notes } = sketcherToValues(fixture('angled'));
  assert.deepEqual(notes, []);
  assert.deepEqual(values.profile, [[0, 247.5, 1], [1, 63.310228]]);
  assert.equal(values.height, 300);
  assert.equal(values.entries, 5);
  assert.equal(values.socket, 'E27');
  assert.equal(values.wing_zone, 250);
  assert.equal(values.max_overhang, 70);
  assert.equal(values.top, 'crease');
  assert.equal(values.z_by, 'flat', 'its true-helix thread kept, the rest flat');
  assert.equal(values.path.length, 7);
});

test('a socket of its own, a scale and a floor lamp', () => {
  const { values, notes } = sketcherToValues({ ...fixture('straight'), sockD: 41, scale: 2, lamp: 'floor' });
  assert.equal(values.socket, 'custom');
  assert.equal(values.socket_d, 41);
  assert.equal(values.profile[0][1], 145, 'radius × scale');
  assert.equal(notes.length, 1);
  assert.match(notes[0], /only the pendant/);
});

test('written back, the sketcher reads the same shade', () => {
  for (const name of ['straight', 'sphere', 'angled', 'halfsphere']) {
    const file = fixture(name);
    const back = valuesToSketcher(sketcherToValues(file).values);
    for (const k of ['H', 'N', 'wingLen', 'maxOver', 'tipGap', 'neckGap', 'wallGap', 'pinGap', 'wingRot', 'grip', 'hide', 'crease', 'topMode', 'zMode', 'sockD', 'sockP', 'sockH', 'eSock', 'pathLen', 'LH', 'mount', 'lamp'])
      assert.equal(back[k], file[k], `${name}: ${k}`);
    const near = (a, b) => Math.abs(a - b) < 1e-5;
    assert.ok(back.profile.every((p, i) => near(p.u, file.profile[i].u) && near(p.r, file.profile[i].r) && !!p.sharp === !!file.profile[i].sharp), name);
    assert.ok(back.wing.every((p, i) => near(p.u, file.wing[i].u) && near(p.l, file.wing[i].l)), name);
    assert.ok(back.path.every((p, i) => near(p.u, file.path[i].u) && near(p.w, file.path[i].w)), name);
  }
});
