// The Print tab: the printer (with the machine settings G-code needs), and estimates and checks
// for the toolpath the recipe prints (`print path`).
//
// A printer profile has the same fields as the engine's `gcode::Profile` (camelCase). The
// defaults are EasyGCode's, so G-code starts out exactly as EasyGCode would write it; changes are
// remembered on this device, per printer.

import { el, fmt } from './dom.js';

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

// maxFlow: the most volumetric flow (mm³/s) the printer melts reliably, for the Print tab's check
// (0: not checked). The P1P's hotend manages about 20 mm³/s with PLA; set your pellet printer's.
export const PRINTERS = {
  p1p: { name: 'Bambu P1P', nozzle: 0.4, defaults: { ...EASYGCODE, mode: 'filament', bed: [256, 256, 256], maxFlow: 20 } },
  pellet: { name: 'Klipper pellet', nozzle: null, defaults: { ...EASYGCODE, mode: 'pellet', bed: [400, 400, 400], maxFlow: 0 } },
};

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
  { key: 'pellet.up', label: 'Upper zone', unit: '°C', min: 0, max: 350, only: 'pellet' },
  { key: 'pellet.mid', label: 'Middle zone', unit: '°C', min: 0, max: 350, only: 'pellet' },
  { key: 'pellet.down', label: 'Lower zone', unit: '°C', min: 0, max: 350, only: 'pellet' },
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

/** A fresh copy of a printer's defaults. */
function defaults(id) {
  return structuredClone(PRINTERS[id].defaults);
}

/** The printer's profile: its defaults, with the settings changed on this device. */
export function loadProfile(id) {
  const profile = defaults(id);
  try {
    const saved = JSON.parse(localStorage.getItem(storageKey(id)) || 'null');
    if (saved && typeof saved === 'object') {
      for (const f of FIELDS) {
        const v = getPath(saved, f.key);
        if (typeof v === typeof getPath(profile, f.key) && (typeof v !== 'number' || Number.isFinite(v))) setPath(profile, f.key, v);
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

/** The printer profile a recipe's `printer "…"` line names. */
export function printerFor(name) {
  return /pellet|klipper/i.test(name || '') ? 'pellet' : 'p1p';
}

/**
 * The printer settings: number fields for the machine, and a reset to EasyGCode's defaults.
 * @param {HTMLElement} host
 * @param {keyof typeof PRINTERS} id
 * @param {any} profile edited in place
 * @param {() => void} onChange after every change (already saved)
 */
export function renderPrinterSettings(host, id, profile, onChange) {
  host.replaceChildren();
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
      const input = /** @type {HTMLInputElement} */ (
        el('input', { type: 'number', id: fieldId, inputMode: 'decimal', step: 'any', value: String(getPath(profile, f.key)) })
      );
      input.addEventListener('change', () => {
        const v = Number(input.value);
        if (input.value.trim() === '' || !Number.isFinite(v)) {
          input.value = String(getPath(profile, f.key));
          return;
        }
        const tidy = Math.min(f.max ?? v, Math.max(f.min ?? v, v));
        input.value = String(tidy);
        setPath(profile, f.key, tidy);
        saveProfile(id, profile);
        onChange();
      });
      row.append(el('span', { className: 'num' }, [input, el('span', { className: 'unit', text: f.unit })]));
    }
    host.append(row);
  }
  const reset = el('button', { className: 'btn', type: 'button', text: 'Back to EasyGCode’s defaults' });
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
 * @param {HTMLElement} statsHost
 * @param {HTMLElement} checksHost
 * @param {{ stats: Record<string, any> } | null} print the engine's PrintInfo
 * @param {keyof typeof PRINTERS} printerId
 * @param {any} profile the printer's settings
 */
export function renderPrint(statsHost, checksHost, print, printerId, profile) {
  statsHost.replaceChildren();
  checksHost.replaceChildren();
  if (!print) {
    statsHost.append(
      el('p', {
        className: 'hint',
        text: 'Nothing to print yet. A recipe prints a toolpath with a line like: print path (where path = vase_spiral(wall, layer: 0.3 mm, width: 0.6 mm), or toolpath(curves, …) for any curves).',
      }),
    );
    return;
  }
  const st = print.stats;
  const cards = [
    [fmt(st.layers), st.layers === 1 ? 'layer' : 'layers'],
    [`${fmt(st.printedHeightMm, 1)} mm`, 'printed height'],
    [duration(st.timeS), st.byFlow ? `at ${fmt(st.flowMm3S, 1)} mm³/s` : `at ${fmt(st.speedMmS)} mm/s`],
    [`${fmt(st.massGPla)} g`, `PLA · ${fmt(st.volumeMm3 / 1000, 1)} cm³`],
    [`${fmt(st.pathLengthMm / 1000, 1)} m`, 'toolpath length'],
    st.byFlow ? [`${fmt(st.speedMmS)} mm/s`, 'nozzle speed (full bead)'] : [`${fmt(st.flowMm3S, 1)} mm³/s`, 'volumetric flow'],
  ];
  for (const [big, small] of cards) statsHost.append(el('div', { className: 'stat' }, [el('b', { text: big }), el('span', { text: small })]));

  const printer = PRINTERS[printerId] || PRINTERS.p1p;
  const [bx, by, bz] = profile.bed;
  const [cx, cy] = profile.center;
  const b = st.bounds;
  const x0 = b.min[0] + cx;
  const x1 = b.max[0] + cx;
  const y0 = b.min[1] + cy;
  const y1 = b.max[1] + cy;
  const onBed = x0 >= 0 && x1 <= bx && y0 >= 0 && y1 <= by;
  // Word joiners keep a range like "65–186" on one line.
  const range = (axis, a, b) => `${axis}\u00a0${fmt(a)}\u2060–\u2060${fmt(b)}`;
  const where = `${range('X', x0, x1)}, ${range('Y', y0, y1)}`;
  const checks = [
    [
      onBed ? 'ok' : 'bad',
      onBed
        ? `On the ${fmt(bx)} × ${fmt(by)} mm bed: ${where}`
        : `Off the ${fmt(bx)} × ${fmt(by)} mm bed (${where}): make it smaller or move the recipe origin in the printer settings`,
    ],
    [st.printedHeightMm <= bz ? 'ok' : 'bad', `${fmt(st.printedHeightMm, 1)} mm tall, the build height is ${fmt(bz)} mm`],
  ];
  // The overhang is measured between the turns of one continuous spiral (vase mode).
  if (st.samplesPerTurn > 0 && st.layers > 1) {
    const overhang = st.maxOverhangDeg;
    checks.push([overhang < 40 ? 'ok' : overhang < 55 ? 'warn' : 'bad', `Steepest overhang ${fmt(overhang, 1)}° (under ~45° prints cleanly)`]);
  }
  if (profile.maxFlow > 0) {
    const q = st.flowMm3S;
    checks.push([
      q <= profile.maxFlow ? 'ok' : q <= profile.maxFlow * 1.15 ? 'warn' : 'bad',
      `Flow up to ${fmt(q, 1)} mm³/s; this printer manages about ${fmt(profile.maxFlow)} mm³/s`,
    ]);
  }
  if (printer.nozzle) {
    const ratio = st.widthMm / printer.nozzle;
    checks.push([ratio >= 1 && ratio <= 2 ? 'ok' : 'warn', `Line width is ${fmt(ratio, 1)}× the ${printer.nozzle} mm nozzle`]);
  }
  for (const [kind, text] of checks) {
    checksHost.append(el('li', {}, [el('span', { className: `dot ${kind === 'ok' ? '' : kind}` }), el('span', { text })]));
  }
}
