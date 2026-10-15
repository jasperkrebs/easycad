// The Print tab: estimates and checks for the toolpath the recipe prints (`print path`).

import { el, fmt } from './dom.js';

export const PRINTERS = {
  p1p: { name: 'Bambu P1P', bed: 256, nozzle: 0.4 },
  pellet: { name: 'Klipper pellet', bed: 400, nozzle: null },
};

/** The printer profile a recipe's `printer "…"` line names. */
export function printerFor(name) {
  return /pellet|klipper/i.test(name || '') ? 'pellet' : 'p1p';
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
 */
export function renderPrint(statsHost, checksHost, print, printerId) {
  statsHost.replaceChildren();
  checksHost.replaceChildren();
  if (!print) {
    statsHost.append(
      el('p', {
        className: 'hint',
        text: 'Nothing to print yet. A recipe prints a toolpath with a line like: print path (where path = vase_spiral(wall, layer: 0.3 mm, width: 0.6 mm)).',
      }),
    );
    return;
  }
  const st = print.stats;
  const cards = [
    [fmt(st.layers), 'layers, one continuous spiral'],
    [`${fmt(st.printedHeightMm, 1)} mm`, 'printed height (whole layers)'],
    [duration(st.timeS), `at ${fmt(st.speedMmS)} mm/s`],
    [`${fmt(st.massGPla)} g`, `PLA · ${fmt(st.volumeMm3 / 1000, 1)} cm³`],
    [`${fmt(st.pathLengthMm / 1000, 1)} m`, 'toolpath length'],
    [`${fmt(st.flowMm3S, 1)} mm³/s`, 'volumetric flow'],
  ];
  for (const [big, small] of cards) statsHost.append(el('div', { className: 'stat' }, [el('b', { text: big }), el('span', { text: small })]));

  const printer = PRINTERS[printerId] || PRINTERS.p1p;
  const b = st.bounds;
  const width = Math.max(b.max[0] - b.min[0], b.max[1] - b.min[1]);
  const overhang = st.maxOverhangDeg;
  const checks = [
    [overhang < 40 ? 'ok' : overhang < 55 ? 'warn' : 'bad', `Steepest overhang ${fmt(overhang, 1)}° (under ~45° prints cleanly)`],
    [width <= printer.bed - 10 ? 'ok' : 'bad', `About ${fmt(width)} mm wide on a ${printer.bed} mm bed`],
  ];
  if (printer.nozzle) {
    const ratio = st.widthMm / printer.nozzle;
    checks.push([ratio >= 1 && ratio <= 2 ? 'ok' : 'warn', `Line width is ${fmt(ratio, 1)}× the ${printer.nozzle} mm nozzle`]);
  }
  for (const [kind, text] of checks) {
    checksHost.append(el('li', {}, [el('span', { className: `dot ${kind === 'ok' ? '' : kind}` }), el('span', { text })]));
  }
}
