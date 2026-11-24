// Tests for web/js/fixture.js (the lamp's fittings for the view, the plan and the render), run
// with `node --test scripts/` (CI does): the standard sizes, where the socket goes on the shade
// and which way the project hangs, the outlines, and the render settings read back safely.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { touching } from '../web/js/wall.js';
import { PLASTICS, cleanMaterial, materialValues, physicalOf } from '../web/js/materials.js';
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
  // An older file's settings (samples, scale, matte) just fall away.
  const r = cleanRender({ light: false, lumens: 1e9, samples: 'x', room: -1, scale: 2, matte: 0.5 });
  assert.equal(r.light, false);
  assert.equal(r.lumens, 5000);
  assert.equal(r.room, 0);
  assert.equal('scale' in r || 'samples' in r || 'matte' in r, false);
  assert.deepEqual(r.final, RENDER_DEFAULTS.final);
  const q = cleanRender({ preview: { size: 99999, samples: 0 }, wall: { on: true, distance: -5 }, camera: { view: 'below', lens: 85, position: [1, 2, 3], target: [0, 0, 'x'] }, surface: 'red' });
  assert.deepEqual(q.preview, { size: 4096, samples: 1 });
  assert.deepEqual(q.wall, { on: true, distance: 0 });
  assert.equal(q.camera.view, 'below');
  assert.equal(q.camera.lens, 85);
  assert.equal(q.camera.position, null, 'a camera without a target is no camera');
  assert.equal(q.surface, RENDER_DEFAULTS.surface);
  const c = cleanRender({ camera: { position: [0.1, 0.2, 0.9], target: [0, 0.1, 0] } });
  assert.deepEqual(c.camera.position, [0.1, 0.2, 0.9]);
});

test("plastics: a part's material reads back safely and overrides its plastic", () => {
  assert.deepEqual(cleanMaterial(null), { plastic: 'pla' });
  assert.deepEqual(cleanMaterial({ plastic: 'nylon-unknown', roughness: 5, ior: 'x' }), { plastic: 'pla', roughness: 1 });
  const v = materialValues({ plastic: 'petg', roughness: 0.3 });
  assert.equal(v.roughness, 0.3);
  assert.equal(v.ior, PLASTICS.petg.ior);
  for (const [key, p] of Object.entries(PLASTICS)) {
    assert.ok(p.label && p.ior >= 1.3 && p.ior <= 1.7 && p.depth > 0, key);
  }
});

test('a look on the physical material: solid bounces, see-through transmits and tints over its depth', () => {
  const solid = physicalOf({ color: '#ffffff', opacity: 1, material: { plastic: 'pla' } });
  assert.equal(solid.transmission, 0);
  assert.equal(solid.thickness, 0);
  assert.equal(solid.attenuationDistance, Infinity);
  const clear = physicalOf({ color: '#e0f0ff', opacity: 0.25, material: { plastic: 'pc' } });
  assert.equal(clear.transmission, 0.75);
  assert.equal(clear.thickness, 1);
  assert.ok(Math.abs(clear.attenuationDistance - PLASTICS.pc.depth / 1000) < 1e-12);
  assert.equal(clear.ior, PLASTICS.pc.ior);
  assert.equal(physicalOf({ color: 'bad', opacity: 1 }).color, '#f2f2f2');
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

test("the socket's thread: the recipes' pitch, crests at the body, right-handed, fading out at its ends", async () => {
  const { socketThread, threadRadius, socketSection } = await import('../web/js/fixture.js');
  const e27 = socketOf(params({ socket: 'E27' }));
  const e14 = socketOf(params({ socket: 'E14' }));
  assert.equal(e27.pitch, 2.5);
  assert.equal(e14.pitch, 2);
  assert.equal(socketOf(params({ socket: 'custom', thread_d: 50, thread_pitch: 3 })).pitch, 3);
  assert.equal(socketOf(params({ socket: 'custom', socket_d: 40, socket_pitch: 2.2 })).pitch, 2.2);
  const t = socketThread(e27);
  assert.equal(t.crest, 20, 'the crests are the body (the shade thread) diameter');
  assert.ok(t.depth > 0.5 && t.depth < 2);
  // Mid-way up: a crest every pitch at one angle, a root half a pitch on.
  const mid = t.z0 + 8 * t.pitch;
  assert.ok(Math.abs(threadRadius(t, 0, mid) - t.crest) < 1e-9);
  assert.ok(Math.abs(threadRadius(t, 0, mid + t.pitch / 2) - t.root) < 1e-9);
  // Right-handed: a quarter turn on (counter-clockwise from above), the crest is a quarter pitch up.
  assert.ok(Math.abs(threadRadius(t, Math.PI / 2, mid + t.pitch / 4) - t.crest) < 1e-9);
  // It fades to the root at both ends, where it meets the body.
  assert.ok(Math.abs(threadRadius(t, 0, t.z0) - t.root) < 1e-9);
  assert.ok(Math.abs(threadRadius(t, 0, t.z1) - t.root) < 1e-9);
  // The section: still closed on the axis, with teeth (a crest a pitch).
  const section = socketSection(e27, 0);
  assert.equal(section[0][0], 0);
  assert.equal(section[section.length - 1][0], 0);
  const crests = section.filter(([r, z]) => z > t.z0 + t.pitch && z < t.z1 - t.pitch && r > t.crest - 0.05).length;
  assert.ok(crests >= Math.floor((t.z1 - t.z0) / t.pitch) - 3, `${crests} crests`);
  // The plan's outline shows them.
  const fx = { ...e27, face: 0, up: 1, flipped: false, part: null };
  assert.ok(fixtureOutline(fx).socket.length > 100);
});

test('taken apart, the fittings come out of the lamp: up past the shade round the bulb, or down out of a shade above', async () => {
  const { fixtureTakenOut } = await import('../web/js/fixture.js');
  // A shade printed throat down and opening up: hung down, the project's turned over (+z down).
  const down = fixtureOf(params({ socket: 'E27' }), shade(23, 20), 'down');
  assert.equal(down.throat, 20);
  assert.equal(fixtureTakenOut(down, 0), down);
  const out = fixtureTakenOut(down, 1);
  // Up in the world is -z in the project: the face moves by the throat, cap, glass and a gap.
  assert.equal(out.face, down.face - (20 + 3 + 83 + 20));
  assert.equal(fixtureTakenOut(down, 0.5).face, down.face - (20 + 3 + 83 + 20) / 2);
  // Hung up: the shade rises from the socket; the fittings come down out of its throat.
  const up = fixtureOf(params({ socket: 'E27' }), shade(23, 20), 'up');
  assert.equal(fixtureTakenOut(up, 1).face, up.face - (20 + 20));
});
