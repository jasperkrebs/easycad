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
  // The spiral disc's defaults in EasyGCode's volumetric flow mode: the feed follows from
  // 20 mm³/s (EasyCAD's spiral disc template).
  'sdisc-flow': {
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
      flowFeed: { enabled: true, rate: 20 },
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
// The same cylinder in flow mode, 8 mm³/s throughout.
cases['lamp-cylinder-flow'] = JSON.parse(JSON.stringify(cases['lamp-cylinder']));
cases['lamp-cylinder-flow'].lamp.flowFeed = { enabled: true, rate: 8 };

// The thread connector: its defaults (EasyGCode index.html), and two variations that exercise a
// brim with its own layer height (the hop over it, and the fan after the helix's first turn on
// the filament printer), a fraction of a turn, a tighter fit, a set line width and no start/end
// blocks. Reference for EasyCAD's thread template (`helix`, `circle(r, tolerance)`, `join`).
const thread = (over) => ({
  project: 'thread',
  printer: pellet(),
  travelFeed: 6000,
  tolerance: 0.01,
  centerX: 125,
  centerY: 125,
  ...over,
});
cases['thread-default'] = thread({
  thread: { size: 'm8', height: 20, fitTolerance: 0, lineWidth: 0, flowRate: 20 },
  brim: { enabled: false, linesOuter: 4, lineWidth: 0, layerHeight: 0, flowRate: 20 },
});
cases['thread-brim'] = thread({
  printer: pellet({
    mode: 'filament',
    endLift: 30,
    filament: { diameter: 1.75, nozzle: 215, bed: 65, fan: 100 },
  }),
  travelFeed: 9000,
  thread: { size: 'm12', height: 15.5, fitTolerance: -0.1, lineWidth: 0, flowRate: 15 },
  brim: { enabled: true, linesOuter: 3, lineWidth: 0, layerHeight: 0.8, flowRate: 10 },
});
cases['thread-bare'] = thread({
  printer: pellet({ includeStartEnd: false }),
  tolerance: 0.02,
  centerX: 100,
  centerY: 110,
  thread: { size: 'm3', height: 6, fitTolerance: 0.05, lineWidth: 1, flowRate: 5 },
  brim: { enabled: true, linesOuter: 2, lineWidth: 1.2, layerHeight: 0, flowRate: 5 },
});

// The spoon: its defaults in volumetric flow mode (20 mm³/s), one printed stick first with a
// thicker stick bead on the filament printer, and one with no stick and no start/end blocks.
// Reference for EasyCAD's spoon template (`spiral(start, turns)`, `center`, `bead`, `stack`).
const spoon = (over, sp) => ({
  project: 'spoon',
  printer: pellet(),
  layerHeight: 0.6,
  lineWidth: 2.0,
  printFeed: 1200,
  travelFeed: 6000,
  centerX: 125,
  centerY: 125,
  ...over,
  spoon: {
    turns: 6,
    startRadius: 0,
    stickLength: 40,
    layers: 3,
    startPoint: 'center',
    stickLineWidth: 0,
    stickLayerHeight: 0,
    stickFeed: 0,
    flowFeed: { enabled: true, rate: 20 },
    ...sp,
  },
});
cases['spoon-flow'] = spoon({}, {});
cases['spoon-stick'] = spoon(
  {
    printer: pellet({
      mode: 'filament',
      endLift: 30,
      filament: { diameter: 1.75, nozzle: 210, bed: 60, fan: 80 },
    }),
    layerHeight: 0.4,
    lineWidth: 1.2,
    travelFeed: 9000,
    centerX: 128,
    centerY: 128,
  },
  {
    turns: 4.5,
    startRadius: 5,
    stickLength: 30,
    layers: 2,
    startPoint: 'stick',
    stickLineWidth: 3,
    stickLayerHeight: 0.8,
    flowFeed: { enabled: true, rate: 12 },
  }
);
cases['spoon-bare'] = spoon(
  { printer: pellet({ includeStartEnd: false }), layerHeight: 0.5, lineWidth: 1.5, centerX: 110, centerY: 120 },
  { turns: 3, startRadius: 2, stickLength: 0, layers: 1, flowFeed: { enabled: true, rate: 10 } }
);

// The bend stool's seat without legs (EasyGCode's default): concentric rings with a staircase
// seam, the first layer outside-in from a lead-in line, a wipe between layers. In volumetric
// flow mode as EasyCAD's template ships; with foaming over four layers at a set feed; with
// foaming, a cool-down below the normal temperatures, a zigzag exit primer, primers at a set
// flow and the print centred elsewhere; and three plain layers without start/end blocks.
// Reference for EasyCAD's bend stool template (`arc(tolerance:)`, `vertices`, `stack(wipe:)`,
// `temperature`, `wait_temperature`, `speed_override`, `extrusion_override`).
const stool = (over, disc) => ({
  project: 'bendstool',
  printer: pellet(),
  materialDensity: 0,
  materialPrice: 0,
  layerHeight: 0.6,
  lineWidth: 2.0,
  printFeed: 1800,
  travelFeed: 6000,
  tolerance: 0.05,
  centerX: 125,
  centerY: 125,
  ...over,
  disc: {
    diameter: 100,
    layers: 1,
    seamStyle: 'staircase',
    dome: 1,
    legs: { enabled: false, seatHeight: 200, width: 40, fillet: 8 },
    attractor: { enabled: false, pos: 0, r1: 15, r2: 35, gap: 1, drop: 0.5 },
    foam: {
      enabled: false,
      tempUp: 230,
      tempMid: 230,
      tempDown: 230,
      extrusionPct: 38,
      exitCool: 0,
      primer1: { length: 40, lineWidth: 2, layerHeight: 0.3, feed: 600, flow: 0 },
      primer2: { length: 60, lines: 1, lineWidth: 2, layerHeight: 0.3, feed: 600, flow: 0 },
    },
    flowFeed: { enabled: false, rate: 20 },
    ...disc,
  },
  brim: { enabled: false, outerStyle: 'normal', base: 'bottom', linesOuter: 3, linesInner: 0, lineWidth: 2, layerHeight: 0.6, feed: 1200, multiplier: 1 },
});
cases['stool-flow'] = stool({}, { flowFeed: { enabled: true, rate: 20 } });
cases['stool-foam'] = stool({}, { layers: 4, foam: { ...stool({}, {}).disc.foam, enabled: true } });
cases['stool-cool'] = stool(
  { layerHeight: 0.5, lineWidth: 1.5, tolerance: 0.04, centerX: 150, centerY: 140 },
  {
    diameter: 61,
    layers: 5,
    foam: {
      enabled: true,
      tempUp: 240,
      tempMid: 235,
      tempDown: 230,
      extrusionPct: 45,
      exitCool: 185,
      primer1: { length: 30, lineWidth: 1.8, layerHeight: 0.4, feed: 600, flow: 5 },
      primer2: { length: 50, lines: 3, lineWidth: 1.6, layerHeight: 0.35, feed: 600, flow: 4 },
    },
    flowFeed: { enabled: true, rate: 15 },
  }
);
cases['stool-bare'] = stool(
  { printer: pellet({ includeStartEnd: false }), layerHeight: 0.4, lineWidth: 1.2, printFeed: 1500, centerX: 100, centerY: 110 },
  { diameter: 30, layers: 3 }
);

// The lampshade: its defaults (E27, a cone with a fillet, width compensation) in volumetric
// flow mode as EasyCAD's template ships; a capped bell on an E14 with layer-height compensation
// and a brim at a set feed; a sphere with a lip, wide edge down, its flow ramping from the
// throat's to the shade's over the fillet; and a dome on a custom thread without compensation,
// a set ramp height and no start/end blocks. Reference for EasyCAD's lampshade template
// (`vase_revolve`, `graph`, `bounds`, `graft`).
const lamp = (ls, over = {}) => ({
  project: 'lamp',
  printer: pellet(),
  lineWidth: 6,
  printFeed: 1200,
  travelFeed: 6000,
  tolerance: 0.1,
  centerX: 125,
  centerY: 125,
  brim: { enabled: false, linesOuter: 4, lineWidth: 6, layerHeight: 2.5, feed: 900, multiplier: 0 },
  ...over,
  lamp: {
    socket: 'e27',
    customDiameter: 40,
    customPitch: 2.5,
    fitTolerance: 0,
    shape: 'cone',
    throatLength: 20,
    fillet: 10,
    bottomDiameter: 150,
    transitionHeight: 52,
    maxAngle: 0,
    sphereDiameter: 200,
    flatRatio: 0.5,
    lip: { enabled: false, length: 10, fillet: 0 },
    orientation: 'throat',
    compMode: 'width',
    compStrength: 100,
    compMaxMult: 2.5,
    flowFeed: { enabled: false, rate: 20, shadeRate: 0, transitionHeight: 0 },
    ...ls,
  },
});
cases['lampshade-flow'] = lamp({ flowFeed: { enabled: true, rate: 20, shadeRate: 0, transitionHeight: 0 } });
cases['lampshade-bell'] = lamp(
  { socket: 'e14', fitTolerance: -0.2, shape: 'arcOut', throatLength: 15, bottomDiameter: 180, transitionHeight: 60, maxAngle: 55, compMode: 'layerHeight', compStrength: 80 },
  { brim: { enabled: true, linesOuter: 3, lineWidth: 5, layerHeight: 2, feed: 900, multiplier: 0 } }
);
cases['lampshade-sphere'] = lamp({
  shape: 'sphere',
  throatLength: 12,
  bottomDiameter: 120,
  sphereDiameter: 220,
  lip: { enabled: true, length: 8, fillet: 0 },
  orientation: 'wide',
  compMaxMult: 2,
  flowFeed: { enabled: true, rate: 18, shadeRate: 30, transitionHeight: 0 },
});
cases['lampshade-dome'] = lamp(
  {
    socket: 'custom',
    customDiameter: 35,
    customPitch: 3,
    shape: 'arcIn',
    throatLength: 10,
    fillet: 25,
    bottomDiameter: 160,
    transitionHeight: 70,
    compMode: 'off',
    flowFeed: { enabled: true, rate: 15, shadeRate: 25, transitionHeight: 30 },
  },
  { printer: pellet({ includeStartEnd: false }), lineWidth: 5, tolerance: 0.08, centerX: 150, centerY: 150 }
);

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
