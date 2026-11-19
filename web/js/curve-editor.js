// A list setting of points ([x, y], or [x, y, corner]) as a graph, like Grasshopper's Graph
// Mapper: the points on a curve through them, each dragged to a new place. Nothing is computed
// while a point moves; letting go is one change. The table and the graph edit the same rows.

import { el } from './dom.js';

const SVG = 'http://www.w3.org/2000/svg';
const W = 320;
const H = 200;
const PAD = { left: 34, right: 10, top: 10, bottom: 22 };

/** @param {string} tag @param {Record<string, string | number>} attrs */
function svg(tag, attrs = {}) {
  const node = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, String(v));
  return node;
}

/**
 * The axis range for some values: 0..1 when they all lie in it (most curves here are
 * "0..1" along something), else their own span with a little room.
 * @param {number[]} vs
 * @returns {[number, number]}
 */
export function axisRange(vs) {
  const finite = vs.filter(Number.isFinite);
  if (!finite.length) return [0, 1];
  const lo = Math.min(...finite);
  const hi = Math.max(...finite);
  if (lo >= 0 && hi <= 1) return [0, 1];
  const span = hi - lo || Math.abs(hi) || 1;
  const pad = span * 0.1;
  return [lo - pad, hi + pad];
}

/**
 * A value rounded to what the graph can show: about a thousandth of the axis.
 * @param {number} v @param {[number, number]} range
 */
export function tidy(v, range) {
  const span = Math.abs(range[1] - range[0]) || 1;
  const places = Math.max(0, 3 - Math.floor(Math.log10(span)));
  return Number(v.toFixed(Math.min(places, 10)));
}

/**
 * A smooth curve through points sorted by x that never overshoots them (a monotone cubic, as
 * the Thread pendant reads its curves), sampled for drawing; a corner (third value 1) splits it.
 * @param {number[][]} rows
 * @returns {number[][][]} pieces of [x, y] samples
 */
export function curvePieces(rows) {
  const pts = rows
    .map((r, i) => ({ x: r[0], y: r[1], sharp: (r[2] ?? 0) !== 0, i }))
    .filter((p) => Number.isFinite(p.x) && Number.isFinite(p.y))
    .sort((a, b) => a.x - b.x || a.i - b.i);
  if (pts.length < 2) return pts.length ? [[[pts[0].x, pts[0].y]]] : [];
  const groups = [];
  let cur = [];
  pts.forEach((p, k) => {
    cur.push(p);
    if (p.sharp && k > 0 && k < pts.length - 1) {
      groups.push(cur);
      cur = [p];
    }
  });
  groups.push(cur);
  return groups.map((g) => monotone(g.map((p) => p.x), g.map((p) => p.y)));
}

/** Fritsch–Carlson: a cubic through each pair of points whose slopes keep it between them. */
function monotone(xs, ys) {
  const n = xs.length;
  if (n < 2) return [[xs[0], ys[0]]];
  const d = [];
  for (let i = 0; i < n - 1; i++) d.push((ys[i + 1] - ys[i]) / Math.max(xs[i + 1] - xs[i], 1e-9));
  const m = new Array(n).fill(0);
  m[0] = d[0];
  m[n - 1] = d[n - 2];
  for (let i = 1; i < n - 1; i++) m[i] = d[i - 1] * d[i] <= 0 ? 0 : (d[i - 1] + d[i]) / 2;
  for (let i = 0; i < n - 1; i++) {
    if (d[i] === 0) {
      m[i] = 0;
      m[i + 1] = 0;
      continue;
    }
    const a = m[i] / d[i];
    const b = m[i + 1] / d[i];
    const s = a * a + b * b;
    if (s > 9) {
      const t = 3 / Math.sqrt(s);
      m[i] = t * a * d[i];
      m[i + 1] = t * b * d[i];
    }
  }
  const out = [];
  for (let i = 0; i < n - 1; i++) {
    const h = xs[i + 1] - xs[i];
    const steps = 16;
    for (let j = i === 0 ? 0 : 1; j <= steps; j++) {
      const t = j / steps;
      const t2 = t * t;
      const t3 = t2 * t;
      const y = (2 * t3 - 3 * t2 + 1) * ys[i] + (t3 - 2 * t2 + t) * h * m[i] + (-2 * t3 + 3 * t2) * ys[i + 1] + (t3 - t2) * h * m[i + 1];
      out.push([xs[i] + t * h, y]);
    }
  }
  return out;
}

/** A number for an axis label: short. */
const label = (v) => String(Number(v.toPrecision(3)));

/**
 * The graph: an SVG of the points on their curve. Dragging a point moves it (between its
 * neighbours along x, inside the axes); letting go calls `onCommit` once.
 * @param {{ items: number[][], name: string, onCommit: () => void }} opts the rows (edited in
 *   place, shared with the table), the setting's label (for screen readers), and the change
 * @returns {{ node: HTMLElement, redraw: () => void }}
 */
export function curveEditor(opts) {
  const root = svg('svg', { viewBox: `0 0 ${W} ${H}`, class: 'curve-graph', role: 'img', 'aria-label': `${opts.name}: graph` });
  const host = el('div', { className: 'curve-editor' }, [/** @type {any} */ (root)]);
  /** @type {[number, number]} */
  let xr = [0, 1];
  /** @type {[number, number]} */
  let yr = [0, 1];
  /** The point being dragged, if any: its axes stay put until it's let go. */
  let dragging = -1;
  const px = (x) => PAD.left + ((x - xr[0]) / (xr[1] - xr[0] || 1)) * (W - PAD.left - PAD.right);
  const py = (y) => H - PAD.bottom - ((y - yr[0]) / (yr[1] - yr[0] || 1)) * (H - PAD.top - PAD.bottom);
  const fromPx = (sx, sy) => [
    xr[0] + ((sx - PAD.left) / (W - PAD.left - PAD.right)) * (xr[1] - xr[0]),
    yr[0] + ((H - PAD.bottom - sy) / (H - PAD.top - PAD.bottom)) * (yr[1] - yr[0]),
  ];

  const redraw = () => {
    const items = opts.items;
    if (dragging < 0) {
      xr = axisRange(items.map((r) => r[0]));
      yr = axisRange(items.map((r) => r[1]));
    }
    root.replaceChildren();
    // The frame and a quarter grid, with the axes' ends.
    for (let q = 0; q <= 4; q++) {
      const gx = PAD.left + (q / 4) * (W - PAD.left - PAD.right);
      const gy = PAD.top + (q / 4) * (H - PAD.top - PAD.bottom);
      root.append(svg('line', { x1: gx, y1: PAD.top, x2: gx, y2: H - PAD.bottom, class: q % 4 ? 'grid' : 'axis' }));
      root.append(svg('line', { x1: PAD.left, y1: gy, x2: W - PAD.right, y2: gy, class: q % 4 ? 'grid' : 'axis' }));
    }
    const text = (x, y, t, anchor) => {
      const node = svg('text', { x, y, 'text-anchor': anchor, class: 'tick' });
      node.textContent = t;
      root.append(node);
    };
    text(PAD.left, H - 6, label(xr[0]), 'start');
    text(W - PAD.right, H - 6, label(xr[1]), 'end');
    text(PAD.left - 4, H - PAD.bottom, label(yr[0]), 'end');
    text(PAD.left - 4, PAD.top + 8, label(yr[1]), 'end');
    for (const piece of curvePieces(items)) {
      root.append(svg('polyline', { points: piece.map(([x, y]) => `${px(x).toFixed(1)},${py(y).toFixed(1)}`).join(' '), class: 'curve' }));
    }
    items.forEach((r, i) => {
      if (!Number.isFinite(r[0]) || !Number.isFinite(r[1])) return;
      const g = svg('g', { class: `handle${i === dragging ? ' active' : ''}${(r[2] ?? 0) !== 0 ? ' sharp' : ''}`, 'data-row': i });
      g.append(svg('circle', { cx: px(r[0]), cy: py(r[1]), r: 14, class: 'hit' }));
      g.append(svg('circle', { cx: px(r[0]), cy: py(r[1]), r: 5, class: 'dot' }));
      root.append(g);
    });
  };

  /** Where a pointer is, in the drawing's own units. */
  const at = (e) => {
    const box = root.getBoundingClientRect();
    return [((e.clientX - box.left) / box.width) * W, ((e.clientY - box.top) / box.height) * H];
  };
  root.addEventListener('pointerdown', (e) => {
    const g = /** @type {Element} */ (e.target).closest?.('.handle');
    if (!g) return;
    dragging = Number(g.getAttribute('data-row'));
    root.setPointerCapture(e.pointerId);
    e.preventDefault();
    redraw();
  });
  root.addEventListener('pointermove', (e) => {
    if (dragging < 0) return;
    const items = opts.items;
    const row = items[dragging];
    if (!row) return;
    let [x, y] = fromPx(...at(e));
    // Between its neighbours along x (the curve's order stays the table's), inside the axes.
    const lo = Math.max(xr[0], dragging > 0 ? items[dragging - 1][0] : -Infinity);
    const hi = Math.min(xr[1], dragging < items.length - 1 ? items[dragging + 1][0] : Infinity);
    x = Math.min(hi, Math.max(lo, x));
    y = Math.min(yr[1], Math.max(yr[0], y));
    row[0] = tidy(x, xr);
    row[1] = tidy(y, yr);
    redraw();
  });
  const end = () => {
    if (dragging < 0) return;
    dragging = -1;
    redraw();
    opts.onCommit();
  };
  root.addEventListener('pointerup', end);
  root.addEventListener('pointercancel', end);
  redraw();
  return { node: host, redraw };
}
