// Tests for web/js/fixture.js (the lamp's fittings for the view, the plan and the render), run
// with `node --test scripts/` (CI does): the standard sizes, where the socket goes on the shade
// and which way the project hangs, the outlines, and the render settings read back safely.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { touching } from '../web/js/wall.js';
import { FITTINGS, RENDER_DEFAULTS, bulbProfile, cleanRender, fixtureOf, fixtureOutline, kelvinColor, socketOf, socketProfile } from '../web/js/fixture.js';

const params = (values) => Object.entries(values).map(([name, value]) => ({ name, value }));

/** A shade's (radius, z) points: a straight start at `r` from z0 for `throat` mm, then out to 80 mm going `dir`. */
function shade(r, throat, dir = 1, z0 = 0, width = 6) {
  const points = [];
  for (let z = 0; z <= throat; z += 2.5) points.push([r, z0 + dir * z]);
  for (let k = 1; k <= 20; k++) points.push([r + 4 * k, z0 + dir * (throat + 3 * k)]);
  return new Map([['main', { points, width }]]);
}

test('the standard fittings: E27 takes the A60 pear, E14 the P45 round', () => {
  assert.equal(socketOf(params({ socket: 'E27' })).bulb, 'A60');
  assert.equal(socketOf(params({ socket: 'E14' })).bulb, 'P45');
  assert.equal(FITTINGS.E27.glass, 60);
  assert.equal(FITTINGS.E14.glass, 45);
  // A60: about 108 mm with its cap; P45: about 80 mm.
  assert.ok(Math.abs(FITTINGS.E27.glassL + 25 - 108) < 3);
  assert.ok(Math.abs(FITTINGS.E14.glassL + 22 - 80) < 3);
  assert.equal(socketOf(params({ shape: 'cone' })), null, 'no socket setting: no fittings');
});

test('a custom socket takes its own diameter and height', () => {
  const lamp = socketOf(params({ socket: 'custom', thread_d: 52 }));
  assert.equal(lamp.body, 52);
  assert.equal(lamp.bulb, 'A60');
  const pendant = socketOf(params({ socket: 'custom', socket_d: 30, socket_h: 70 }));
  assert.equal(pendant.body, 30);
  assert.equal(pendant.height, 70);
  assert.equal(pendant.bulb, 'P45');
});

test("the socket sits on the straight start; hung down, its face at the start's lower end", () => {
  // The shade prints throat down and opens upward (as both templates do by default).
  const parts = shade(23, 20);
  const down = fixtureOf(params({ socket: 'E27' }), parts, 'down');
  // Hanging down, the project is turned over: its +z points down, the face at the throat's top.
  assert.equal(down.flipped, true);
  assert.equal(down.up, -1);
  assert.equal(down.face, 20);
  assert.equal(down.part, 'main');
  const up = fixtureOf(params({ socket: 'E27' }), parts, 'up');
  assert.equal(up.flipped, false);
  assert.equal(up.up, 1);
  assert.equal(up.face, 0);
});

test('a shade that already goes down hangs as it is', () => {
  const parts = shade(23, 20, -1);
  const fx = fixtureOf(params({ socket: 'E27' }), parts, 'down');
  assert.equal(fx.flipped, false);
  assert.equal(fx.face, -20);
});

test('another shade further out on the start is not taken for the throat', () => {
  const parts = shade(23, 20);
  // A second shade screwed onto the first one's outside, a line and a half further out.
  parts.set('outer', { points: [[32, 0], [32, 10], [32, 20], [32, 30], [32, 40], [32, 50]], width: 6 });
  assert.equal(fixtureOf(params({ socket: 'E27' }), parts).part, 'main');
});

test('the outlines: closed profiles that start and end on the axis, the bulb below the socket', () => {
  for (const kind of ['E27', 'E14']) {
    const f = socketOf(params({ socket: kind }));
    for (const profile of [bulbProfile(f), socketProfile(f)]) {
      assert.equal(profile[0][0], 0);
      assert.equal(profile[profile.length - 1][0], 0);
      for (const [r, z] of profile) assert.ok(Number.isFinite(r) && Number.isFinite(z) && r >= 0);
    }
    const widest = Math.max(...bulbProfile(f).map(([r]) => r));
    assert.ok(Math.abs(2 * widest - f.glass) < 0.5, `${kind}: the glass is ${f.glass} mm across`);
    const fx = { ...f, face: 10, up: 1, flipped: false, part: null };
    const o = fixtureOutline(fx);
    assert.ok(Math.max(...o.bulb.map(([, z]) => z)) <= 10, 'the bulb hangs below the face');
    assert.ok(Math.min(...o.socket.map(([, z]) => z)) >= 10 - 1e-9, 'the holder rises from the face');
    assert.ok(o.cable.every((edge) => edge[1][1] > edge[0][1]), 'the cable goes up');
  }
});

test('colour temperature: warm is red-heavy, daylight near white', () => {
  const [r, g, b] = kelvinColor(2700);
  assert.equal(r, 1);
  assert.ok(g < 0.8 && b < 0.5);
  const day = kelvinColor(6500);
  assert.ok(day.every((v) => v > 0.9));
});

test('render settings read back safely', () => {
  assert.deepEqual(cleanRender(null), RENDER_DEFAULTS);
  assert.deepEqual(cleanRender('nonsense'), RENDER_DEFAULTS);
  const r = cleanRender({ light: false, lumens: 1e9, samples: 'x', room: -1, scale: 2 });
  assert.equal(r.light, false);
  assert.equal(r.lumens, 5000);
  assert.equal(r.samples, RENDER_DEFAULTS.samples);
  assert.equal(r.room, 0);
  assert.equal(r.scale, 2);
});

test('a vase wall: each turn sits on the one below, the first has nothing under it, the last nothing over it', () => {
  // Three turns of a helix, 2 mm a turn, 24 points a turn, 4 mm beads.
  const pts = [];
  const per = 24;
  for (let k = 0; k <= 3 * per; k++) {
    const a = (2 * Math.PI * k) / per;
    pts.push(50 * Math.cos(a), 50 * Math.sin(a), 2 + (2 * k) / per);
  }
  const n = pts.length / 3;
  const widths = new Float32Array(n).fill(4);
  widths[0] = 0;
  const heights = new Float32Array(n).fill(2);
  const { above, below } = touching(pts, widths, heights);
  assert.equal(below[per / 2], 0, 'the first turn sits on the bed');
  assert.equal(above[per / 2], 1);
  assert.equal(below[per + per / 2], 1);
  assert.equal(above[per + per / 2], 1);
  assert.equal(above[2 * per + per / 2], 0, 'nothing on the last turn');
  assert.equal(below[2 * per + per / 2], 1);
  // A turn stepped out by more than half a bead doesn't sit on the one below.
  const out = pts.slice();
  for (let k = 2 * per + 1; k < n; k++) {
    out[3 * k] *= 60 / 50;
    out[3 * k + 1] *= 60 / 50;
  }
  assert.equal(touching(out, widths, heights).below[2 * per + per / 2], 0);
});
