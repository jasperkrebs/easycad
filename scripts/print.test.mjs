// Tests for web/js/print.js (the printers), run with `node --test scripts/` (CI does): the Ginger
// G1's ranges and presets, and the print checks its wiki calls for.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { G1, PRINTERS, applyMaterial, changeSettings, describePrinter, loadProfile, machineGuide, materialOf, printChecks, printerFor } from '../web/js/print.js';

/** Print statistics as the engine reports them: a 100 mm vase spiral by default. */
const stats = (over = {}) => ({
  bounds: { min: [-50, -50, 0], max: [50, 50, 60] },
  printedHeightMm: 60,
  samplesPerTurn: 120,
  travelLengthMm: 0,
  layers: 100,
  maxOverhangDeg: 10,
  flowMm3S: 20,
  widthMm: 2,
  layerMm: 0.6,
  maxFanPct: 0,
  ...over,
});

const find = (checks, pattern) => checks.find(([, text]) => pattern.test(text));

test('recipes name the G1 in old and new words', () => {
  for (const name of ['Ginger G1', 'Klipper pellet', 'ginger', 'G1 pellet']) assert.equal(printerFor(name), 'pellet', name);
  for (const name of ['Bambu P1P 0.4', '', undefined, 'G10 printer']) assert.equal(printerFor(name), 'p1p', String(name));
  assert.equal(PRINTERS.pellet.recipeName, 'Ginger G1');
});

test("the G1's defaults are its bed and its heater limits", () => {
  const p = loadProfile('pellet');
  assert.deepEqual(p.bed, [1000, 1000, 1000]);
  assert.equal(p.maxFlow, 200);
  assert.deepEqual([p.pellet.minExtrudeTemp, p.pellet.maxTemp, p.pellet.maxBed], [160, 280, 110]);
  // EasyGCode's machine settings, so the G-code stays EasyGCode's.
  assert.deepEqual([p.center, p.travelSpeed, p.pellet.up, p.pellet.pa], [[125, 125], 100, 210, 0.3]);
  // The nozzle is chosen per project, not set on the printer.
  assert.equal(p.nozzle, undefined);
});

test("settings stay inside the G1's own ranges", () => {
  const p = loadProfile('pellet');
  const { changed, notes } = changeSettings('pellet', p, { 'pellet.up': 300, 'pellet.down': 150, 'pellet.bed': 120, travelSpeed: 400 });
  assert.deepEqual(changed, ['pellet.up', 'pellet.down', 'pellet.bed', 'travelSpeed']);
  assert.deepEqual([p.pellet.up, p.pellet.down, p.pellet.bed, p.travelSpeed], [280, 160, 110, 300]);
  assert.equal(notes.length, 4);
  assert.match(notes[0], /clamped to 280 \(range 160\.\.280\)/);
  assert.match(changeSettings('pellet', p, { nozzle: 5 }).notes[0], /not a setting of the Ginger G1/);
  // The P1P keeps the plain ranges.
  const f = loadProfile('p1p');
  changeSettings('p1p', f, { travelSpeed: 400 });
  assert.equal(f.travelSpeed, 400);
  assert.match(changeSettings('p1p', f, { nozzle: 1 }).notes[0], /not a setting of the Bambu P1P/);
});

test("material presets are Ginger Slicer's profiles", () => {
  const p = loadProfile('pellet');
  assert.equal(materialOf('pellet', p), undefined, "EasyGCode's 210 °C isn't one of them");
  const r = applyMaterial('pellet', p, 'colorfabb foam pla');
  assert.deepEqual(r.notes, []);
  assert.deepEqual([p.pellet.up, p.pellet.mid, p.pellet.down, p.pellet.bed, p.maxFlow], [185, 210, 210, 70, 500]);
  assert.equal(materialOf('pellet', p)?.name, 'colorFabb FOAM PLA');
  assert.match(describePrinter('pellet', p), /Material preset: colorFabb FOAM PLA\./);
  changeSettings('pellet', p, { 'pellet.bed': 55 });
  assert.match(describePrinter('pellet', p), /Material preset: none \(own settings\)\./);
  assert.match(applyMaterial('pellet', p, 'Unobtainium').notes[0], /no material preset "Unobtainium" \(one of: Generic PLA, /);
  assert.match(applyMaterial('p1p', loadProfile('p1p'), 'Generic PLA').notes[0], /the Bambu P1P has no material presets/);
  // Every preset is inside the G1's limits.
  for (const m of G1.materials) {
    for (const t of [m.up, m.mid, m.down]) assert.ok(t >= 160 && t <= 280, m.name);
    assert.ok(m.bed <= 110 && m.maxFlow > 0, m.name);
  }
});

test('the G1 checks low flow, pressure advance in vase mode and the fan', () => {
  const p = loadProfile('pellet');
  let checks = printChecks(stats(), 'pellet', p);
  assert.equal(find(checks, /bed/)[0], 'ok');
  assert.deepEqual(find(checks, /this printer manages about 200/)[0], 'ok');
  assert.equal(find(checks, /barrel inlet can plug/)[0], 'warn');
  assert.equal(find(checks, /nozzle/), undefined, 'no nozzle to check a line against');
  assert.equal(find(checks, /pressure advance 0[.,]30/)[0], 'warn');
  assert.equal(find(checks, /Part fan/), undefined);
  // Vase mode is one continuous line, a regular spiral or not (the vessel's spiralize isn't);
  // layers joined by travel moves aren't.
  assert.equal(find(printChecks(stats({ samplesPerTurn: 0 }), 'pellet', p), /pressure advance/)[0], 'warn');
  assert.equal(find(printChecks(stats({ samplesPerTurn: 0, travelLengthMm: 105 }), 'pellet', p), /pressure advance/), undefined);
  assert.equal(find(printChecks(stats({ samplesPerTurn: 0, layers: 1 }), 'pellet', p), /pressure advance/), undefined);

  // A cooler upper zone is what low flow needs; no pressure advance for a spiral.
  changeSettings('pellet', p, { 'pellet.up': 190, 'pellet.pa': 0 });
  checks = printChecks(stats(), 'pellet', p);
  assert.equal(find(checks, /upper zone 20 °C below the others/)[0], 'ok');
  assert.equal(find(checks, /pressure advance/), undefined);
  // Above the low-flow range there's nothing to say about it.
  assert.equal(find(printChecks(stats({ flowMm3S: 180 }), 'pellet', p), /low flow|barrel inlet/i), undefined);

  // Over the max flow: a warning, then a problem.
  assert.equal(find(printChecks(stats({ flowMm3S: 220 }), 'pellet', p), /manages about 200/)[0], 'warn');
  assert.equal(find(printChecks(stats({ flowMm3S: 260 }), 'pellet', p), /manages about 200/)[0], 'bad');
  // A fan event (or the setting) above ~70 %.
  assert.equal(find(printChecks(stats({ maxFanPct: 100 }), 'pellet', p), /Part fan up to 100 %/)[0], 'warn');
  // Off the bed when the origin's too close to its edge.
  assert.equal(find(printChecks(stats({ bounds: { min: [-200, -50, 0], max: [200, 50, 60] } }), 'pellet', p), /Off the/)[0], 'bad');
});

test('the P1P keeps its own checks', () => {
  const checks = printChecks(stats({ widthMm: 0.45, layerMm: 0.2, flowMm3S: 12 }), 'p1p', loadProfile('p1p'));
  assert.equal(find(checks, /Line width is 1[.,]1× the 0[.,]4 mm nozzle$/)[0], 'ok');
  assert.equal(find(checks, /manages about 20 mm³\/s$/)[0], 'ok');
  for (const g1 of [/barrel inlet/, /pressure advance/, /Layers/, /Part fan/]) assert.equal(find(checks, g1), undefined, String(g1));
});

test("the agent's guide carries the G1's facts", () => {
  const guide = machineGuide();
  for (const fact of [/1000 × 1000 × 1000 mm/, /mostly runs the 5 mm/, /5 mm: layer 2[.,]5, line 5[.,]5/, /160–280 °C/, /bed up to 110 °C/, /colorFabb FOAM PLA 185\/210\/210, bed 70, 500 mm³\/s/, /stepper too far in past/]) {
    assert.match(guide, fact);
  }
});

test("the bead is EasyGCode's: a rectangle with a half circle each side; blends show their extremes", async () => {
  const { beadArea, beadExtremes } = await import('../web/js/beads.js');
  // 2 × 0.6 mm: 1.4 × 0.6 plus a 0.6 mm circle.
  assert.equal(beadArea(2, 0.6), 1.4 * 0.6 + Math.PI * 0.09);
  // Two beads (and travel, 0): both, smallest first.
  const two = beadExtremes([0, 2, 2, 3, 3, 0], [0, 0.6, 0.6, 0.3, 0.3, 0]);
  assert.equal(two.blended, false);
  assert.deepEqual(
    two.beads.map((b) => [b.w, b.h]),
    [
      [3, 0.3],
      [2, 0.6],
    ],
  );
  // A smooth blend from 2 × 0.6 to 3 × 0.3: only the two ends.
  const n = 50;
  const ws = Array.from({ length: n }, (_, i) => 2 + i / (n - 1));
  const hs = Array.from({ length: n }, (_, i) => 0.6 - (0.3 * i) / (n - 1));
  const blend = beadExtremes(ws, hs);
  assert.equal(blend.blended, true);
  assert.deepEqual(blend.beads.map((b) => [b.w, b.h]).sort(), [
    [2, 0.6],
    [3, 0.3],
  ]);
  // Widths and heights varying apart: the narrowest, widest, flattest and tallest.
  const apart = beadExtremes([2, 2.5, 3, 3.5, 4, 2.2], [0.5, 0.4, 0.6, 0.3, 0.5, 0.7]);
  assert.deepEqual(new Set(apart.beads.map((b) => `${b.w}x${b.h}`)), new Set(['2x0.5', '4x0.5', '3.5x0.3', '2.2x0.7']));
  // A blend whose flattest step is a hair narrower than its widest: one bead, not two.
  const hair = beadExtremes([3, 3.3, 3.6, 3.9, 4.2, 4.49, 4.5], [1.3, 1.4, 1.5, 1.6, 1.8, 2, 2]);
  assert.deepEqual(hair.beads.map((b) => `${b.w}x${b.h}`), ['3x1.3', '4.5x2']);
  assert.deepEqual(beadExtremes([0, 0], [0, 0]).beads, []);
});
