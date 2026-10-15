// Generates golden G-code fixtures by running EasyGCode's own generator (gcode.js) headlessly,
// the way EasyGCode's CLAUDE.md describes. EasyCAD's tests compare their G-code with these.
//
//   node scripts/easygcode-fixtures.cjs <path to an easygcode checkout>
//
// Writes fixtures/easygcode/<case>.json (the EasyGCode config) and <case>.gcode, and records the
// EasyGCode commit in fixtures/easygcode/SOURCE. EasyGCode itself is only read, never changed.

const fs = require('node:fs');
const path = require('node:path');
const { execSync } = require('node:child_process');

const src = process.argv[2];
if (!src) {
  console.error('usage: node scripts/easygcode-fixtures.cjs <path to easygcode>');
  process.exit(2);
}
global.window = {};
require(path.resolve(src, 'geometry.js'));
require(path.resolve(src, 'gcode.js'));
const { GcodeGen } = global.window;

const pellet = (over = {}) => ({
  mode: 'pellet',
  multiplier: 1,
  includeStartEnd: true,
  endLift: 50,
  filament: { diameter: 1.75, nozzle: 205, bed: 60, fan: 100 },
  pellet: { up: 210, mid: 210, down: 210, bed: 50, pa: 0.3, purge: 5000, fan: 0 },
  ...over,
});

// The lampshade generator with a straight "cone" whose two ends are the same size is a plain
// cylinder printed in vase mode: the reference for EasyCAD's `vase_spiral`. Its points per turn
// come from the chord tolerance; this one gives exactly 192, the number of points EasyCAD's
// `circle` has at full quality.
const LAMP_R = (40 + 0.8) / 2; // throat: (thread diameter + line width) / 2
const lampTolerance = Number((LAMP_R * (1 - Math.cos(Math.PI / 191.5))).toFixed(6));
const lampSteps = Math.max(24, Math.ceil((2 * Math.PI) / (2 * Math.acos(1 - lampTolerance / LAMP_R))));
if (lampSteps !== 192) throw new Error(`the lamp case needs 192 points per turn, got ${lampSteps}`);

// The spiral disc's own defaults (EasyGCode index.html), plus two variations that exercise the
// filament start/end blocks, stacked layers, a full-spacing band, a one-sided falloff and an even
// bump count (rounded up to odd).
const cases = {
  'sdisc-default': {
    project: 'sdisc',
    printer: pellet(),
    layerHeight: 0.6,
    lineWidth: 2.0,
    printFeed: 1200,
    travelFeed: 6000,
    tolerance: 0.05,
    centerX: 125,
    centerY: 125,
    sdisc: {
      outerRadius: 60,
      attractorRadius: 35,
      maxSpacingPct: 200,
      fullBand: 0,
      falloffIn: 15,
      falloffOut: 15,
      bumps: 21,
      layers: 1,
      flowFeed: { enabled: false, rate: 20 },
    },
  },
  'sdisc-filament': {
    project: 'sdisc',
    printer: pellet({
      mode: 'filament',
      multiplier: 0.95,
      endLift: 30,
      filament: { diameter: 1.75, nozzle: 210, bed: 65, fan: 80 },
    }),
    layerHeight: 0.3,
    lineWidth: 0.6,
    printFeed: 1800,
    travelFeed: 9000,
    tolerance: 0.05,
    centerX: 128,
    centerY: 128,
    sdisc: {
      outerRadius: 16,
      attractorRadius: 9,
      maxSpacingPct: 250,
      fullBand: 2,
      falloffIn: 5,
      falloffOut: 3,
      bumps: 15,
      layers: 2,
      flowFeed: { enabled: false, rate: 20 },
    },
  },
  'sdisc-bare': {
    project: 'sdisc',
    printer: pellet({ includeStartEnd: false }),
    layerHeight: 0.4,
    lineWidth: 1.2,
    printFeed: 900,
    travelFeed: 6000,
    tolerance: 0.05,
    centerX: 100,
    centerY: 100,
    sdisc: {
      outerRadius: 20,
      attractorRadius: 10,
      maxSpacingPct: 300,
      fullBand: 0,
      falloffIn: 20,
      falloffOut: 0,
      bumps: 8,
      layers: 3,
      flowFeed: { enabled: false, rate: 20 },
    },
  },
  'lamp-cylinder': {
    project: 'lamp',
    printer: pellet(),
    lineWidth: 0.8,
    printFeed: 1200,
    travelFeed: 6000,
    tolerance: lampTolerance,
    centerX: 125,
    centerY: 125,
    brim: { enabled: false },
    lamp: {
      socket: 'custom',
      customDiameter: 40,
      customPitch: 0.5, // the layer height
      fitTolerance: 0,
      bottomDiameter: 2 * LAMP_R, // the same as the throat: a cylinder
      throatLength: 10,
      transitionHeight: 10,
      shape: 'cone',
      fillet: 0,
      maxAngle: 0,
      orientation: 'throat',
      lip: { enabled: false },
      compMode: 'off',
      flowFeed: { enabled: false },
    },
  },
};

const out = path.join(__dirname, '..', 'fixtures', 'easygcode');
fs.mkdirSync(out, { recursive: true });
for (const [name, cfg] of Object.entries(cases)) {
  const r = GcodeGen.generate(cfg);
  fs.writeFileSync(path.join(out, `${name}.json`), JSON.stringify(cfg, null, 2) + '\n');
  fs.writeFileSync(path.join(out, `${name}.gcode`), r.gcode);
  console.log(`${name}: ${r.gcode.split('\n').length} lines, warnings: ${JSON.stringify(r.warnings)}`);
}
let commit = 'unknown';
try {
  commit = execSync('git rev-parse HEAD', { cwd: src }).toString().trim();
} catch {
  // not a git checkout
}
fs.writeFileSync(path.join(out, 'SOURCE'), `jasperkrebstc/easygcode ${commit}\n`);
