// The Print tab: the printer (with the machine settings G-code needs), and estimates and checks
// for the toolpath the recipe prints (`print path`).
//
// A printer profile has the same fields as the engine's `gcode::Profile` (camelCase). The
// defaults are EasyGCode's, so G-code starts out exactly as EasyGCode would write it; changes are
// remembered on this device, per printer.

import { el, fmt, numberField, parseNumber } from './dom.js';

/** EasyGCode's machine settings (travel 6000 mm/min, end lift 50 mm, the bed centre at 125/125). */
const EASYGCODE = {
  multiplier: 1,
  includeStartEnd: true,
  endLift: 50,
  travelSpeed: 100,
  center: [125, 125],
  filament: { diameter: 1.75, nozzle: 205, bed: 60, fan: 100 },
  pellet: { up: 210, mid: 210, down: 210, bed: 50, pa: 0.3, purge: 5000, fan: 0 },
};

/**
 * The Ginger Additive G1, a pellet printer running Klipper, from its wiki
 * (wiki.gingeradditive.com), whose figures come from the machine's printer.cfg and Ginger
 * Slicer's shipped profiles. docs/printers.md has the pages.
 */
export const G1 = {
  /** Heater and motion limits from printer.cfg: Klipper stops a print that asks for more. */
  limits: { minExtrudeTemp: 160, maxTemp: 280, maxBed: 110, maxSpeed: 300 },
  /**
   * The interchangeable nozzles, with the layer height and line width Ginger's process profiles
   * use (layers about half the nozzle: thicker ones drag the nozzle on the layer below), and the
   * most flow a nozzle manages where the wiki gives it (mm³/s). For the agent's advice: the
   * nozzle is chosen per project (the owner mostly runs the 5 mm), not a printer setting.
   */
  nozzles: [
    { d: 1, layer: 0.6, line: 1.2, maxFlow: 150 },
    { d: 1.8, layer: 0.9, line: 1.9 },
    { d: 3, layer: 1.3, line: 3.2, maxFlow: 300 },
    { d: 5, layer: 2.5, line: 5.5 },
    { d: 8, layer: 4, line: 9.5, maxFlow: 500 },
  ],
  /**
   * Below about 150 mm³/s the barrel inlet can plug (it bites around 100): the pellets passing
   * through are what cools it. The fix is a cooler upper (feeding) zone, never a hotter one.
   */
  lowFlow: 150,
  /** Part cooling above about 70 % can pull the nozzle zone down into a heating error. */
  maxFan: 70,
  /** Ginger Slicer's shipped material profiles: zones top to bottom, bed (°C), max flow (mm³/s). */
  materials: [
    ['Generic PLA', 200, 200, 200, 50, 200],
    ['FormFutura PLA', 200, 200, 200, 50, 200],
    ['SmartMaterials PLA', 185, 185, 185, 50, 200],
    ['Azure PLA', 215, 210, 210, 60, 200],
    ['Ginger rPLA', 205, 200, 200, 55, 180],
    ['colorFabb FOAM PLA', 185, 210, 210, 70, 500],
    ['Generic PETG', 220, 240, 240, 80, 200],
    ['Azure PETG', 210, 210, 210, 80, 250],
    ['FormFutura rPETG', 215, 235, 235, 80, 200],
    ['FormFutura rPETG 10GF', 205, 225, 225, 80, 200],
    ['FormFutura rPETG 20GF', 220, 235, 245, 80, 200],
    ['Generic PETG GF', 220, 235, 245, 80, 200],
    ['Polymaker PETG 10GF', 190, 230, 240, 80, 200],
    ['Polymaker PETG 20GF', 220, 230, 240, 80, 200],
    ['Generic PP', 170, 240, 230, 80, 200],
    ['Generic ABS / ASA', 220, 240, 260, 80, 200],
    ['Generic HIPS', 210, 230, 230, 80, 200],
  ].map(([name, up, mid, down, bed, maxFlow]) => ({ name: /** @type {string} */ (name), up, mid, down, bed, maxFlow })),
};

const { minExtrudeTemp, maxTemp, maxBed, maxSpeed } = G1.limits;

// maxFlow: the most volumetric flow (mm³/s) the printer melts reliably, for the Print tab's check
// (0: not checked). The P1P's hotend manages about 20 mm³/s with PLA; Ginger's profiles run the
// G1 at 200. recipeName: what a recipe's `printer "…"` line says for it. limits: ranges the
// machine itself sets for some settings. The G1's heater limits also go to the engine, which
// refuses G-code (machine events included) that would ask for more.
export const PRINTERS = {
  p1p: { name: 'Bambu P1P', recipeName: 'Bambu P1P 0.4', nozzle: 0.4, defaults: { ...EASYGCODE, mode: 'filament', bed: [256, 256, 256], maxFlow: 20 } },
  pellet: {
    name: 'Ginger G1',
    recipeName: 'Ginger G1',
    nozzle: null, // interchangeable, chosen per project
    machine: G1,
    limits: {
      'pellet.up': [minExtrudeTemp, maxTemp],
      'pellet.mid': [minExtrudeTemp, maxTemp],
      'pellet.down': [minExtrudeTemp, maxTemp],
      'pellet.bed': [0, maxBed],
      travelSpeed: [1, maxSpeed],
    },
    defaults: {
      ...EASYGCODE,
      mode: 'pellet',
      bed: [1000, 1000, 1000],
      maxFlow: 200,
      pellet: { ...EASYGCODE.pellet, minExtrudeTemp, maxTemp, maxBed },
    },
  },
};

/** The bed the pellet printer had before it was the G1, replaced when found in saved settings. */
const OLD_PELLET_BED = [400, 400, 400];

/** The fields of the printer settings, by path into the profile. `only`: for one mode. */
const FIELDS = [
  { key: 'bed.0', label: 'Bed width', unit: 'mm', min: 10, max: 5000 },
  { key: 'bed.1', label: 'Bed depth', unit: 'mm', min: 10, max: 5000 },
  { key: 'bed.2', label: 'Build height', unit: 'mm', min: 1, max: 5000 },
  { key: 'center.0', label: 'Recipe origin at X', unit: 'mm', min: -5000, max: 5000 },
  { key: 'center.1', label: 'Recipe origin at Y', unit: 'mm', min: -5000, max: 5000 },
  { key: 'travelSpeed', label: 'Travel speed', unit: 'mm/s', min: 1, max: 2000 },
  { key: 'maxFlow', label: 'Max flow (0: no check)', unit: 'mm³/s', min: 0, max: 2000 },
  { key: 'multiplier', label: 'Extrusion multiplier', unit: '×', min: 0.01, max: 5 },
  { key: 'endLift', label: 'Lift at the end', unit: 'mm', min: 0, max: 1000 },
  { key: 'includeStartEnd', label: 'Start and end G-code', kind: 'toggle' },
  { key: 'filament.diameter', label: 'Filament diameter', unit: 'mm', min: 0.6, max: 4.9, only: 'filament' },
  { key: 'filament.nozzle', label: 'Nozzle temperature', unit: '°C', min: 0, max: 350, only: 'filament' },
  { key: 'filament.bed', label: 'Bed temperature', unit: '°C', min: 0, max: 150, only: 'filament' },
  { key: 'filament.fan', label: 'Part fan', unit: '%', min: 0, max: 100, only: 'filament' },
  { key: 'pellet.up', label: 'Upper zone (feeding)', unit: '°C', min: 0, max: 350, only: 'pellet' },
  { key: 'pellet.mid', label: 'Middle zone (melting)', unit: '°C', min: 0, max: 350, only: 'pellet' },
  { key: 'pellet.down', label: 'Lower zone (nozzle)', unit: '°C', min: 0, max: 350, only: 'pellet' },
  { key: 'pellet.bed', label: 'Bed temperature', unit: '°C', min: 0, max: 150, only: 'pellet' },
  { key: 'pellet.pa', label: 'Pressure advance', unit: '', min: 0, max: 2, only: 'pellet' },
  { key: 'pellet.purge', label: 'Purge amount', unit: '', min: 0, max: 100000, only: 'pellet' },
  { key: 'pellet.fan', label: 'Part fan', unit: '%', min: 0, max: 100, only: 'pellet' },
];

const storageKey = (id) => `easycad.printer.${id}`;

/** @param {any} obj @param {string} key a path like 'filament.fan' or 'bed.0' */
function getPath(obj, key) {
  return key.split('.').reduce((o, k) => (o == null ? undefined : o[k]), obj);
}

function setPath(obj, key, value) {
  const keys = key.split('.');
  const last = /** @type {string} */ (keys.pop());
  keys.reduce((o, k) => o[k], obj)[last] = value;
}

/** A field's range on this printer: the machine's own limits where it has them. */
function range(id, f) {
  const [min, max] = PRINTERS[id]?.limits?.[f.key] ?? [f.min, f.max];
  return { min, max };
}

/** A fresh copy of a printer's defaults. */
function defaults(id) {
  return structuredClone(PRINTERS[id].defaults);
}

/**
 * The printer's profile: its defaults, with the settings changed on this device (kept inside
 * today's ranges).
 */
export function loadProfile(id) {
  const profile = defaults(id);
  try {
    const saved = JSON.parse(localStorage.getItem(storageKey(id)) || 'null');
    if (saved && typeof saved === 'object') {
      // Saved before the pellet printer was the G1: its old bed size wasn't the user's choice.
      if (id === 'pellet' && JSON.stringify(saved.bed) === JSON.stringify(OLD_PELLET_BED)) delete saved.bed;
      for (const f of FIELDS) {
        if (f.only && f.only !== profile.mode) continue;
        const v = getPath(saved, f.key);
        if (typeof v !== typeof getPath(profile, f.key) || (typeof v === 'number' && !Number.isFinite(v))) continue;
        if (typeof v === 'number') {
          const { min, max } = range(id, f);
          setPath(profile, f.key, Math.min(max ?? v, Math.max(min ?? v, v)));
        } else {
          setPath(profile, f.key, v);
        }
      }
    }
  } catch {
    // Unreadable storage: the defaults.
  }
  return profile;
}

function saveProfile(id, profile) {
  try {
    localStorage.setItem(storageKey(id), JSON.stringify(profile));
  } catch {
    // Storage can be unavailable (private browsing); the settings last until the page closes.
  }
}

/**
 * The printer profile a recipe's `printer "…"` line names. Older projects name the G1 "Klipper
 * pellet"; it's the only Klipper printer so far.
 */
export function printerFor(name) {
  return /ginger|\bg1\b|pellet|klipper/i.test(name || '') ? 'pellet' : 'p1p';
}

/** The G1's material preset these settings match, if any. */
export function materialOf(id, profile) {
  const p = profile.pellet;
  return (PRINTERS[id]?.machine?.materials ?? []).find(
    (m) => m.up === p.up && m.mid === p.mid && m.down === p.down && m.bed === p.bed && m.maxFlow === profile.maxFlow,
  );
}

/**
 * Sets the zones, bed and max flow from one of Ginger Slicer's material profiles (the G1 only),
 * and saves them on this device.
 * @param {keyof typeof PRINTERS} id
 * @param {any} profile edited in place
 * @param {string} name
 * @returns {{ changed: string[], notes: string[] }}
 */
export function applyMaterial(id, profile, name) {
  const materials = PRINTERS[id]?.machine?.materials ?? [];
  const m = materials.find((x) => x.name.toLowerCase() === String(name).trim().toLowerCase());
  if (!m) {
    const which = materials.length ? `one of: ${materials.map((x) => x.name).join(', ')}` : `none: the ${PRINTERS[id].name} has no material presets`;
    return { changed: [], notes: [`there's no material preset "${name}" (${which})`] };
  }
  return changeSettings(id, profile, { 'pellet.up': m.up, 'pellet.mid': m.mid, 'pellet.down': m.down, 'pellet.bed': m.bed, maxFlow: m.maxFlow });
}

/**
 * Changes settings by key (the agent's set_printer), as the Print tab's fields do: checked,
 * clamped to each field's range, and saved on this device.
 * @param {keyof typeof PRINTERS} id
 * @param {any} profile edited in place
 * @param {Record<string, unknown>} patch key (as in FIELDS) → new value
 * @returns {{ changed: string[], notes: string[] }}
 */
export function changeSettings(id, profile, patch) {
  const changed = [];
  const notes = [];
  for (const [key, raw] of Object.entries(patch)) {
    const f = FIELDS.find((x) => x.key === key);
    if (!f || (f.only && f.only !== profile.mode)) {
      notes.push(`\`${key}\` is not a setting of the ${PRINTERS[id].name}`);
      continue;
    }
    if (f.kind === 'toggle') {
      if (typeof raw !== 'boolean') {
        notes.push(`\`${key}\` is on or off: true or false`);
        continue;
      }
      setPath(profile, key, raw);
    } else {
      const v = typeof raw === 'number' || (typeof raw === 'string' && raw.trim()) ? Number(raw) : NaN;
      if (!Number.isFinite(v)) {
        notes.push(`\`${key}\` needs a number`);
        continue;
      }
      const { min, max } = range(id, f);
      const tidy = Math.min(max ?? v, Math.max(min ?? v, v));
      if (tidy !== v) notes.push(`\`${key}\` clamped to ${tidy} (range ${min}..${max})`);
      setPath(profile, key, tidy);
    }
    changed.push(key);
  }
  if (changed.length) saveProfile(id, profile);
  return { changed, notes };
}

/**
 * The printers, the chosen one and all its settings, as text for the agent.
 * @param {keyof typeof PRINTERS} id
 * @param {any} profile
 */
export function describePrinter(id, profile) {
  const all = Object.entries(PRINTERS).map(([k, p]) => `${k} (${p.name}, ${p.defaults.mode})`);
  const fields = FIELDS.filter((f) => !f.only || f.only === profile.mode).map(
    (f) => `${f.key} = ${getPath(profile, f.key)}${f.unit ? ` ${f.unit}` : ''} (${f.label})`,
  );
  const material = PRINTERS[id].machine ? ` Material preset: ${materialOf(id, profile)?.name ?? 'none (own settings)'}.` : '';
  return `${PRINTERS[id].name} (${id}). Printers: ${all.join(', ')}. Its machine settings on this device, by set_printer key: ${fields.join('; ')}.${material}`;
}

/** What the agent should know about the G1, for its (cached) instructions. */
export function machineGuide() {
  const nozzles = G1.nozzles.map((n) => `${n.d} mm: layer ${n.layer}, line ${n.line}${n.maxFlow ? `, up to ~${n.maxFlow} mm³/s` : ''}`);
  return `The pellet printer is a Ginger Additive G1 (Klipper, 3 heating zones, E in mm³), and every pellet template is made for it:
- Build volume 1000 × 1000 × 1000 mm. The recipe origin sits at X125 Y125 (EasyGCode's); a big print needs it moved. The bed's sensor is at its centre (X500 Y500): a large solid bottom right over it can fool the bed into its 110 °C cutoff.
- Nozzles are interchangeable and chosen per project (the user mostly runs the 5 mm; a project's notes may say which), with Ginger's layer height and line width: ${nozzles.join('; ')}. Layers thicker than about half the nozzle drag it on the layer below.
- Flow: Ginger's profiles run 200 mm³/s (up to 500 on big nozzles with the right material). Below about ${G1.lowFlow} mm³/s (and really around 100) the barrel inlet can plug, because the passing pellets are what cool it: at low flow, run the upper (feeding) zone 10–40 °C below the other two, never hotter. Layer time, not flow, is usually what limits a good print: thick beads need time to cool.
- Zones: upper (feeding, Klipper's \`extruder\`), middle (melting, \`extruder1\`), lower (nozzle, \`extruder2\`), each ${minExtrudeTemp}–${maxTemp} °C (Klipper won't extrude below ${minExtrudeTemp}); bed up to ${maxBed} °C. Material presets (set_printer \`material\`, from Ginger Slicer): ${G1.materials.map((m) => `${m.name} ${m.up}/${m.mid}/${m.down}, bed ${m.bed}, ${m.maxFlow} mm³/s`).join('; ')}.
- Pressure advance 0 for vase-mode spirals (otherwise a repeating texture on the wall). Part fan at most about ${G1.maxFan} %. Travel up to ${maxSpeed} mm/s.
- If it stops with "stepper too far in past", the G-code has too many tiny segments: raise the recipe's chord tolerance.`;
}

/**
 * The printer settings: number fields for the machine, a material preset on the G1, and a reset
 * to the defaults.
 * @param {HTMLElement} host
 * @param {keyof typeof PRINTERS} id
 * @param {any} profile edited in place
 * @param {() => void} onChange after every change (already saved)
 */
export function renderPrinterSettings(host, id, profile, onChange) {
  host.replaceChildren();
  const materials = PRINTERS[id].machine?.materials ?? [];
  if (materials.length) {
    const select = /** @type {HTMLSelectElement} */ (el('select', { id: 'printer-material' }));
    select.append(el('option', { value: '', text: 'Own settings' }));
    for (const m of materials) select.append(el('option', { value: m.name, text: m.name }));
    select.value = materialOf(id, profile)?.name ?? '';
    select.addEventListener('change', () => {
      if (!select.value) return;
      applyMaterial(id, profile, select.value);
      renderPrinterSettings(host, id, profile, onChange);
      onChange();
    });
    host.append(el('div', { className: 'setting choice' }, [el('label', { htmlFor: 'printer-material', text: 'Material (Ginger Slicer’s profiles)' }), select]));
  }
  for (const f of FIELDS) {
    if (f.only && f.only !== profile.mode) continue;
    const fieldId = `printer-${f.key.replace('.', '-')}`;
    const row = el('div', { className: `setting ${f.kind || 'number'}` }, [el('label', { htmlFor: fieldId, text: f.label })]);
    if (f.kind === 'toggle') {
      const box = /** @type {HTMLInputElement} */ (el('input', { type: 'checkbox', id: fieldId, className: 'switch', checked: getPath(profile, f.key) === true }));
      box.addEventListener('change', () => {
        setPath(profile, f.key, box.checked);
        saveProfile(id, profile);
        onChange();
      });
      row.append(box);
    } else {
      const { min, max } = range(id, f);
      const { input } = numberField({ id: fieldId, value: String(getPath(profile, f.key)) });
      input.addEventListener('change', () => {
        const v = parseNumber(input.value);
        if (input.value.trim() === '' || !Number.isFinite(v)) {
          input.value = String(getPath(profile, f.key));
          return;
        }
        const tidy = Math.min(max ?? v, Math.max(min ?? v, v));
        input.value = String(tidy);
        setPath(profile, f.key, tidy);
        saveProfile(id, profile);
        if (materials.length) {
          const select = /** @type {HTMLSelectElement | null} */ (host.querySelector('#printer-material'));
          if (select) select.value = materialOf(id, profile)?.name ?? '';
        }
        onChange();
      });
      row.append(el('span', { className: 'num' }, [input, el('span', { className: 'unit', text: f.unit })]));
    }
    host.append(row);
  }
  if (PRINTERS[id].machine) {
    host.append(
      el('p', {
        className: 'hint',
        text: 'The start G-code (EasyGCode’s) uses the G1’s saved bed mesh rather than probing for each print: run BED_LEVEL after a nozzle change, after moving the machine, and now and then. The limits and material presets come from Ginger’s wiki (wiki.gingeradditive.com).',
      }),
    );
  }
  const reset = el('button', { className: 'btn', type: 'button', text: 'Back to the defaults' });
  reset.addEventListener('click', () => {
    Object.assign(profile, defaults(id));
    try {
      localStorage.removeItem(storageKey(id));
    } catch {
      // nothing saved, nothing to remove
    }
    renderPrinterSettings(host, id, profile, onChange);
    onChange();
  });
  host.append(el('div', { className: 'buttons' }, [reset]));
}

function duration(seconds) {
  const m = Math.round(seconds / 60);
  return m < 60 ? `${m} min` : `${Math.floor(m / 60)} h ${String(m % 60).padStart(2, '0')}`;
}

/**
 * The checks for a toolpath on a printer, as [kind, text] with kind 'ok', 'warn' or 'bad'.
 * @param {Record<string, any>} st the engine's print statistics
 * @param {keyof typeof PRINTERS} printerId
 * @param {any} profile the printer's settings
 * @returns {[string, string][]}
 */
export function printChecks(st, printerId, profile) {
  const printer = PRINTERS[printerId] || PRINTERS.p1p;
  const machine = printer.machine;
  const [bx, by, bz] = profile.bed;
  const [cx, cy] = profile.center;
  const b = st.bounds;
  const x0 = b.min[0] + cx;
  const x1 = b.max[0] + cx;
  const y0 = b.min[1] + cy;
  const y1 = b.max[1] + cy;
  const onBed = x0 >= 0 && x1 <= bx && y0 >= 0 && y1 <= by;
  // Word joiners keep a range like "65–186" on one line.
  const span = (axis, a, z) => `${axis} ${fmt(a)}⁠–⁠${fmt(z)}`;
  const where = `${span('X', x0, x1)}, ${span('Y', y0, y1)}`;
  /** @type {[string, string][]} */
  const checks = [
    [
      onBed ? 'ok' : 'bad',
      onBed
        ? `On the ${fmt(bx)} × ${fmt(by)} mm bed: ${where}`
        : `Off the ${fmt(bx)} × ${fmt(by)} mm bed (${where}): make it smaller or move the recipe origin in the printer settings`,
    ],
    [st.printedHeightMm <= bz ? 'ok' : 'bad', `${fmt(st.printedHeightMm, 1)} mm tall, the build height is ${fmt(bz)} mm`],
  ];
  // The overhang is measured between the turns of a regular spiral, point by point.
  const spiral = st.samplesPerTurn > 0 && st.layers > 1;
  // Vase mode: one continuous line over several layers (a regular spiral or not).
  const continuous = st.layers > 1 && (spiral || st.travelLengthMm === 0);
  if (spiral) {
    const overhang = st.maxOverhangDeg;
    checks.push([overhang < 40 ? 'ok' : overhang < 55 ? 'warn' : 'bad', `Steepest overhang ${fmt(overhang, 1)}° (under ~45° prints cleanly)`]);
  }
  const q = st.flowMm3S;
  if (profile.maxFlow > 0) {
    const ceiling = profile.maxFlow;
    checks.push([
      q <= ceiling ? 'ok' : q <= ceiling * 1.15 ? 'warn' : 'bad',
      `Flow up to ${fmt(q, 1)} mm³/s; this printer manages about ${fmt(ceiling)} mm³/s`,
    ]);
  }
  if (machine && q > 0 && q < machine.lowFlow) {
    const p = profile.pellet;
    const below = Math.min(p.mid, p.down) - p.up;
    checks.push(
      below >= 10
        ? ['ok', `Low flow (under ~${machine.lowFlow} mm³/s), with the upper zone ${fmt(below)} °C below the others, as the G1 needs`]
        : [
            'warn',
            `Flow up to ${fmt(q, 1)} mm³/s: below ~${machine.lowFlow} mm³/s the G1's barrel inlet can plug (it bites around 100). Run the upper (feeding) zone 10–40 °C below the other two`,
          ],
    );
  }
  // A fixed nozzle (the P1P's) is checked against the line width; the G1's is chosen per project.
  if (printer.nozzle) {
    const ratio = st.widthMm / printer.nozzle;
    checks.push([ratio >= 1 && ratio <= 2 ? 'ok' : 'warn', `Line width is ${fmt(ratio, 1)}× the ${fmt(printer.nozzle, 1)} mm nozzle`]);
  }
  if (machine) {
    if (continuous && profile.pellet.pa > 0) {
      checks.push(['warn', `A vase-mode spiral with pressure advance ${fmt(profile.pellet.pa, 2)}: set it to 0 for vase mode (otherwise it leaves a repeating texture on the wall)`]);
    }
    const fan = Math.max(profile.pellet.fan, st.maxFanPct || 0);
    if (fan > machine.maxFan) {
      checks.push(['warn', `Part fan up to ${fmt(fan)} %: keep it under ~${machine.maxFan} % on the G1 (more can cool the nozzle zone into a heating error that stops the print)`]);
    }
  }
  return checks;
}

/** A part's statistics as cards: layers, height, time, material, length and speed or flow. */
function statCards(st) {
  const cards = [
    [fmt(st.layers), st.layers === 1 ? 'layer' : 'layers'],
    [`${fmt(st.printedHeightMm, 1)} mm`, 'printed height'],
    [duration(st.timeS), st.byFlow ? `at ${fmt(st.flowMm3S, 1)} mm³/s` : `at ${fmt(st.speedMmS)} mm/s`],
    [`${fmt(st.massGPla)} g`, `PLA · ${fmt(st.volumeMm3 / 1000, 1)} cm³`],
    [`${fmt(st.pathLengthMm / 1000, 1)} m`, 'toolpath length'],
    st.byFlow ? [`${fmt(st.speedMmS)} mm/s`, 'nozzle speed (full bead)'] : [`${fmt(st.flowMm3S, 1)} mm³/s`, 'volumetric flow'],
  ];
  return cards.map(([big, small]) => el('div', { className: 'stat' }, [el('b', { text: big }), el('span', { text: small })]));
}

const checkItem = ([kind, text]) => el('li', {}, [el('span', { className: `dot ${kind === 'ok' ? '' : kind}` }), el('span', { text })]);

/**
 * The print estimate and checks: for the one part a recipe prints, or for each of its parts.
 * @param {HTMLElement} statsHost
 * @param {HTMLElement} checksHost
 * @param {{ name: string, stats: Record<string, any> }[]} prints the engine's PrintInfo list
 * @param {keyof typeof PRINTERS} printerId
 * @param {any} profile the printer's settings
 */
export function renderPrint(statsHost, checksHost, prints, printerId, profile) {
  statsHost.replaceChildren();
  checksHost.replaceChildren();
  if (!prints.length) {
    statsHost.append(
      el('p', {
        className: 'hint',
        text: 'Nothing to print yet. A recipe prints a toolpath with a line like: print path (where path = vase_spiral(wall, layer: 0.3 mm, width: 0.6 mm), or toolpath(curves, …) for any curves).',
      }),
    );
    return;
  }
  if (prints.length === 1) {
    statsHost.append(...statCards(prints[0].stats));
    checksHost.append(...printChecks(prints[0].stats, printerId, profile).map(checkItem));
    return;
  }
  // Several parts (a container and its lid): each its own G-code file, estimate and checks.
  for (const part of prints) {
    statsHost.append(
      el('h3', { className: 'part-title', text: part.name }),
      ...statCards(part.stats),
      el('ul', { className: 'checks part-checks' }, printChecks(part.stats, printerId, profile).map(checkItem)),
    );
  }
}
