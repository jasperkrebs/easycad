// The bead's cross-section, drawn to scale in the Print tab. The bead model is EasyGCode's (and
// the G-code's): a rectangle (w − h) × h with a half circle of diameter h on each side, area
// (w − h)·h + π·(h/2)². A toolpath can lay many beads; where they blend smoothly from one to
// another only the extremes are drawn, so the few that matter can be compared side by side.

import { el } from './dom.js';

const SVG = 'http://www.w3.org/2000/svg';

/** mm for people: up to two decimals, as typed. */
const mm = (n, digits = 2) => Number(n).toLocaleString(undefined, { maximumFractionDigits: digits });

/** The bead's cross-section in mm². */
export function beadArea(w, h) {
  return (w - h) * h + Math.PI * (h / 2) ** 2;
}

/**
 * The beads a toolpath lays (line width and layer height per point, 0 for travel), each once
 * (to 0.01 mm). Up to four are all shown; more means they blend from one to another, and then
 * only the extremes are kept: the narrowest, widest, flattest and tallest (any within 1 % of
 * another counted once).
 * @param {ArrayLike<number>} widths
 * @param {ArrayLike<number>} heights
 * @returns {{ beads: Array<{ w: number, h: number, area: number }>, blended: boolean }}
 */
export function beadExtremes(widths, heights) {
  /** @type {Map<string, { w: number, h: number }>} */
  const seen = new Map();
  for (let i = 0; i < widths.length; i++) {
    const w = Math.round(widths[i] * 100) / 100;
    const h = Math.round(heights[i] * 100) / 100;
    if (!(w > 0 && h > 0)) continue;
    seen.set(`${w}x${h}`, { w, h });
  }
  let all = [...seen.values()];
  const blended = all.length > 4;
  if (blended) {
    const by = (key, sign) => all.reduce((best, b) => (sign * (b[key] - best[key]) < 0 ? b : best));
    all = [...new Set([by('w', 1), by('w', -1), by('h', 1), by('h', -1)])];
    // Extremes within 1 % of each other are the same bead (a blend's last step, 4.49 by 4.5 mm).
    const same = (a, b) => Math.abs(a.w - b.w) <= 0.01 * Math.max(a.w, b.w) && Math.abs(a.h - b.h) <= 0.01 * Math.max(a.h, b.h);
    all = all.sort((a, b) => b.w - a.w || b.h - a.h).filter((b, i, list) => !list.slice(0, i).some((k) => same(k, b)));
  }
  const beads = all.map((b) => ({ ...b, area: beadArea(b.w, b.h) })).sort((a, b) => a.area - b.area || a.w - b.w);
  return { beads, blended };
}

/** The stadium outline of a bead, w × h mm, drawn at `s` px per mm. */
function stadium(w, h, s) {
  const r = (Math.min(w, h) / 2) * s;
  const [W, H] = [w * s, h * s];
  return `M ${r} 0 H ${W - r} A ${r} ${r} 0 0 1 ${W - r} ${H} H ${r} A ${r} ${r} 0 0 1 ${r} 0 Z`;
}

/**
 * Draws each part's beads side by side, all at one scale so they compare: the outline, dashed
 * lines where the rectangle meets the half circles, and the size and area.
 * @param {HTMLElement} host
 * @param {Array<{ name: string, beads: Array<{ w: number, h: number, area: number }>, blended: boolean }>} groups
 */
export function renderBeads(host, groups) {
  host.replaceChildren();
  const shown = groups.filter((g) => g.beads.length);
  if (!shown.length) {
    host.append(el('p', { className: 'hint', text: 'No toolpath to show beads for.' }));
    return;
  }
  const beads = shown.flatMap((g) => g.beads);
  const maxW = Math.max(...beads.map((b) => b.w));
  const maxH = Math.max(...beads.map((b) => b.h));
  // One scale for every bead: the widest fits 140 px, the tallest 70.
  const s = Math.min(140 / maxW, 70 / maxH);
  for (const g of shown) {
    const row = el('div', { className: 'bead-row' });
    for (const b of g.beads) {
      const [W, H] = [b.w * s, b.h * s];
      const svg = document.createElementNS(SVG, 'svg');
      svg.setAttribute('width', String(Math.ceil(W + 2)));
      svg.setAttribute('height', String(Math.ceil(H + 2)));
      svg.setAttribute('viewBox', `-1 -1 ${W + 2} ${H + 2}`);
      svg.setAttribute('role', 'img');
      svg.setAttribute('aria-label', `bead ${mm(b.w)} by ${mm(b.h)} mm`);
      const outline = document.createElementNS(SVG, 'path');
      outline.setAttribute('d', stadium(b.w, b.h, s));
      outline.setAttribute('class', 'bead-shape');
      svg.append(outline);
      if (b.w > b.h) {
        for (const x of [(b.h / 2) * s, (b.w - b.h / 2) * s]) {
          const line = document.createElementNS(SVG, 'line');
          for (const [k, v] of Object.entries({ x1: x, x2: x, y1: 0, y2: H })) line.setAttribute(k, String(v));
          line.setAttribute('class', 'bead-split');
          svg.append(line);
        }
      }
      row.append(
        el('figure', { className: 'bead' }, [
          svg,
          el('figcaption', { text: `${mm(b.w)} × ${mm(b.h)} mm · ${mm(b.area, 1)} mm²` }),
        ]),
      );
    }
    const title = [shown.length > 1 ? g.name : '', g.blended ? 'blends between these (the extremes)' : ''].filter(Boolean).join(': ');
    host.append(el('div', { className: 'bead-group' }, [...(title ? [el('p', { className: 'bead-title', text: title })] : []), row]));
  }
}
