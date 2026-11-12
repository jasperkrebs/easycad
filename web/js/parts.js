// The lamp builder's parts on screen (docs/lamp-builder.md): the Plan (the assembly's
// cross-section, every part as wide as its bead), its add buttons (one per way a new part can go
// on a free port: inside or outside the wall by where it sits, up or down by its arrow), the
// selected part's bar (its shape, which way it goes, its switches, its settings, remove), and the
// values a new part starts with. Nothing here edits the recipe: every change is handed to app.js,
// which goes through the engine's commands (addPart, removePart, setting values).

import { el } from './dom.js';
import { applies } from './settings.js';

const SVG = 'http://www.w3.org/2000/svg';
/** Colours for the parts, in order (EasyGCode's palette: blue, green, amber, violet, teal, pink). */
const COLORS = ['#4f9dff', '#2bd9a0', '#ffb454', '#b58cff', '#3fd0d4', '#ff7ab6'];
/** Points per part drawn in the plan, at most. */
const PLAN_POINTS = 900;
/** An add button's size on screen (px): a comfortable touch target. */
const BUTTON = 40;
/** Room (px) kept round the drawing: labels on the left, add buttons on the right. */
const INSET = { top: 14, right: 62, bottom: 14, left: 70 };
/** Room (px) the selected part's bar takes at the bottom. */
const BAR = 60;
/** How long a new part's straight start is unless the recipe says otherwise (the shade's default). */
const HOLD = 20;

/**
 * Which parts the engine says run into each other (`a` and `b` run into each other …).
 * @param {any[]} diagnostics
 * @returns {Set<string>}
 */
export function collidingParts(diagnostics) {
  const out = new Set();
  for (const d of diagnostics) {
    const m = /^`([^`]+)` and `([^`]+)` run into each other/.exec(d.message);
    if (m) {
      out.add(m[1]);
      out.add(m[2]);
    }
  }
  return out;
}

/**
 * Each printed part's toolpath as the plan draws it: (radius, height) where it's shown, thinned
 * out, with its bead width; print aids (a brim) left out, as they aren't part of the object.
 * Keyed by part (print) name.
 * @param {any} result the engine's result
 * @returns {Map<string, { points: number[][], width: number }>}
 */
export function sections(result) {
  const out = new Map();
  const { points, starts, widths, aids } = result.paths;
  const owner = result.parts?.paths || [];
  for (let k = 0; k + 1 < starts.length; k++) {
    const print = result.prints[owner[k]];
    if (!print) continue;
    const [a, b] = [starts[k], starts[k + 1]];
    const step = Math.max(1, Math.ceil((b - a) / PLAN_POINTS));
    const entry = out.get(print.name) || { points: [], width: 0 };
    let wsum = 0;
    let wn = 0;
    for (let i = a; i < b; i += step) {
      if (aids && aids[i]) continue;
      entry.points.push([Math.hypot(points[3 * i], points[3 * i + 1]), points[3 * i + 2]]);
      if (widths && widths[i] > 0) {
        wsum += widths[i];
        wn++;
      }
    }
    entry.width = Math.max(entry.width, wn ? wsum / wn : 1);
    out.set(print.name, entry);
  }
  return out;
}

/**
 * Whether any part's beads lie on a radius over a stretch of height (a face taken there).
 * @param {Map<string, { points: number[][], width: number }>} parts
 * @param {number} r
 * @param {number} za
 * @param {number} zb
 */
function taken(parts, r, za, zb) {
  const [lo, hi] = za < zb ? [za, zb] : [zb, za];
  for (const { points, width } of parts.values()) {
    const near = 0.5 * width;
    for (const [pr, pz] of points) if (pz >= lo && pz <= hi && Math.abs(pr - r) < near) return true;
  }
  return false;
}

/**
 * Where a new part can go: every port with a straight section ([radius, height, inward, length,
 * socket, side]) whose face is free for a straight start somewhere along it, with how far along
 * that is (0: flush with its end; further on where a part already sits there, as a long tube
 * takes several), and the ways it can go (the component's `direction` choices).
 * @param {any} result
 * @param {Map<string, { points: number[][], width: number }>} parts
 * @param {Set<string>} [hidden] parts hidden from the view: no add buttons on them
 * @param {number} [hold] a new part's straight start (mm)
 * @returns {{ port: any, along: number }[]}
 */
export function freePorts(result, parts, hidden = new Set(), hold = HOLD) {
  const out = [];
  for (const port of result.ports || []) {
    const [r, z, inward, length] = port.value;
    if (port.value.length < 4 || !(length > 0) || hidden.has(port.part)) continue;
    const need = Math.min(hold, length);
    // A millimetre at a time along the section, the first stretch a straight start fits (with a
    // little play at either end, so it doesn't touch what's there).
    for (let a = 0; a + need <= length + 1e-9; a += 1) {
      if (!taken(parts, r, z + inward * (a - 2), z + inward * (a + need + 2))) {
        out.push({ port, along: a });
        break;
      }
    }
  }
  return out;
}

/** The component's `@ask` setting named `name`, if any. */
/**
 * How a part sits on the thread it's screwed onto: the port it's on, which way that straight
 * section runs from its end (`inward`, +1 up), how far along it sits (`along`, mm), how much of
 * its own straight start screws on (`hold`), and how far along it can go (`max`). null for a part
 * on the socket (whose thread is the socket's), or with no such settings.
 * @param {any} result
 * @param {string} name
 */
export function threadOf(result, name) {
  const value = (key) => result.params.find((p) => p.name === key)?.value;
  const mount = value(`${name}.mount`);
  const port = (result.ports || []).find((p) => p.name === mount);
  const along = Number(value(`${name}.along`));
  if (!port || port.value.length < 4 || port.value[4] > 0.5 || !Number.isFinite(along)) return null;
  const joint = Number(value('joint')) || 0;
  let hold = joint > 0 ? joint : Number(value(`${name}.start_len`)) || 0;
  if (value(`${name}.shape`) === 'tube') hold = Math.min(hold, Number(value(`${name}.length`)) || hold);
  const limit = result.params.find((p) => p.name === `${name}.along`)?.max ?? 1000;
  const max = Math.max(0, Math.min(limit, port.value[3] - hold));
  return { port, inward: port.value[2] < 0 ? -1 : 1, along, hold, max };
}

/**
 * A part and every part mounted on it, directly or further out (they move with it).
 * @param {any} result
 * @param {string} name
 */
export function withMounted(result, name) {
  const out = [name];
  const parentOf = (part) => {
    const v = result.params.find((p) => p.name === `${part}.mount`)?.value;
    return typeof v === 'string' && v.includes('.') ? v.split('.')[0] : null;
  };
  const names = (result.partList || []).map((p) => p.name);
  for (let grew = true; grew; ) {
    grew = false;
    for (const n of names) {
      if (!out.includes(n) && out.includes(String(parentOf(n)))) {
        out.push(n);
        grew = true;
      }
    }
  }
  return out;
}

/**
 * The thread's pitch where a part prints: its toolpath's layer height (the most common one).
 * @param {any} result
 * @param {number} index the part's place among the prints
 */
export function pitchOf(result, index) {
  const { starts, heights } = result.paths || {};
  const owner = result.parts?.paths || [];
  const seen = new Map();
  for (let k = 0; heights && starts && k + 1 < starts.length; k++) {
    if (owner[k] !== index) continue;
    for (let i = starts[k]; i < starts[k + 1]; i++) {
      const h = Math.round(heights[i] * 100) / 100;
      if (h > 0) seen.set(h, (seen.get(h) || 0) + 1);
    }
  }
  return [...seen.entries()].sort((a, b) => b[1] - a[1])[0]?.[0] || 0;
}

/**
 * The assembly taken apart, a step per part screwed onto another (the socket's stays): the part
 * furthest out first, then inwards, so every part comes off a part still in place. Each turns
 * off its thread (`travel`: how far along it sat plus its straight start, at `pitch` a turn,
 * `out`: the way off, against the way the section runs) and then moves on without turning by
 * `gap` (its straight start: as long as the thread was), so the exploded parts stand apart.
 * @param {any} result
 * @returns {{ name: string, parent: string | null, out: number, travel: number, gap: number, pitch: number }[]}
 */
export function assemblySteps(result) {
  const names = (result.partList || []).map((p) => p.name);
  const prints = (result.prints || []).map((p) => p.name);
  const parentOf = (part) => {
    const v = result.params.find((p) => p.name === `${part}.mount`)?.value;
    return typeof v === 'string' && v.includes('.') ? v.split('.')[0] : null;
  };
  const depth = (part) => {
    let d = 0;
    for (let p = parentOf(part); p && d < names.length; p = parentOf(p)) d++;
    return d;
  };
  const steps = [];
  for (const name of names) {
    const info = threadOf(result, name);
    const pitch = pitchOf(result, prints.indexOf(name));
    if (!info || !(pitch > 0)) continue;
    steps.push({ name, parent: parentOf(name), out: -info.inward, travel: info.along + info.hold, gap: info.hold, pitch, depth: depth(name), at: names.indexOf(name) });
  }
  steps.sort((a, b) => b.depth - a.depth || b.at - a.at);
  return steps.map(({ name, parent, out, travel, gap, pitch }) => ({ name, parent, out, travel, gap, pitch }));
}

/**
 * Where every part is at `u` of the way through taking the assembly apart (0: assembled, 1:
 * exploded): each step takes an equal share of the time, turning its part off its thread and
 * then moving it on. Only the part coming off turns; once off, a part keeps the angle it came
 * off at and only rides along (up or down) with what it was mounted on as that comes off in
 * turn. By part name: `angle` in degrees (counter-clockwise from above), `dz` in mm.
 * @param {ReturnType<typeof assemblySteps>} steps
 * @param {number} u
 */
export function assemblyMotion(steps, u) {
  const n = steps.length;
  /** @type {Map<string, { angle: number, dz: number }>} */
  const own = new Map();
  steps.forEach((step, i) => {
    const s = Math.min(1, Math.max(0, u * n - i));
    const dist = s * (step.travel + step.gap);
    const turned = Math.min(dist, step.travel);
    own.set(step.name, { angle: (step.out * 360 * turned) / step.pitch, dz: step.out * dist });
  });
  const parent = new Map(steps.map((s) => [s.name, s.parent]));
  /** @type {Map<string, { angle: number, dz: number }>} */
  const out = new Map();
  for (const step of steps) {
    // Its own turn only; the moves of everything under it, which it rides along with.
    const angle = own.get(step.name)?.angle ?? 0;
    let dz = 0;
    const seen = new Set();
    for (let p = /** @type {string | null | undefined} */ (step.name); p && own.has(p) && !seen.has(p); p = parent.get(p)) {
      seen.add(p);
      dz += own.get(p)?.dz ?? 0;
    }
    out.set(step.name, { angle, dz });
  }
  return out;
}

const askOf = (component, name) => (component?.ask || []).find((p) => p.name === name);

/** The part a mark belongs to: its settings are the part's (`main.fillet`). */
function markPart(mark, partNames) {
  const first = String(mark.settings?.[0] ?? '');
  const dot = first.indexOf('.');
  const name = dot > 0 ? first.slice(0, dot) : '';
  return partNames.has(name) ? name : null;
}

// ---------------------------------------------------------------------------------------------
// The view: fitted to the drawing, then zoomed and panned by hand (wheel, pinch, drag); a
// double-click or the Fit button fits it again.
// ---------------------------------------------------------------------------------------------

const view = { zoom: 1, panX: 0, panY: 0 };
/** The last drawing's scale (px per mm) and each part's group, for sliding a part. */

/** What the plan last drew, so a resize, a zoom or a pan can draw it again. */
let drawn = /** @type {{ host: HTMLElement, result: any, handlers: PlanHandlers } | null} */ (null);

/** Fits the plan to its drawing again (forgets the zoom and pan). */
export function fitPlan() {
  view.zoom = 1;
  view.panX = 0;
  view.panY = 0;
  if (drawn && !drawn.host.hidden) draw();
}

/**
 * @typedef {{
 *   selected?: string | null,
 *   hidden?: Set<string>,
 *   colorOf?: (name: string) => string,
 *   onSelect: (name: string | null) => void,
 *   onMark: (mark: any, part: string | null) => void,
 *   onAdd: (add: { port: any | null, values: Record<string, unknown> }) => void,
 *   onSet: (name: string, values: Record<string, unknown>) => void,
 *   onOpen: (name: string) => void,
 *   onRemove: (name: string) => void,
 *   offsets?: Map<string, number>,
 * }} PlanHandlers
 */

/**
 * Draws the plan: the assembly cut through its axis (both halves), each part in its colour (red
 * where it runs into another; faint while hidden), its name on the left, the marks that lead to
 * settings, the add buttons on every free port, and the selected part's bar.
 * @param {HTMLElement} host
 * @param {any} result
 * @param {PlanHandlers} handlers
 */
export function renderPlan(host, result, handlers) {
  drawn = { host, result, handlers };
  if (!host.dataset.wired) wire(host);
  draw();
}

/** The pointer and wheel handling, once per host: zoom, pan, and a tap on nothing deselects. */
function wire(host) {
  host.dataset.wired = '1';
  /** @type {Map<number, { x: number, y: number }>} */
  const pointers = new Map();
  let moved = 0;
  let pinch = 0;
  const local = (e) => {
    const box = host.getBoundingClientRect();
    return { x: e.clientX - box.left, y: e.clientY - box.top };
  };
  const zoomAt = (x, y, factor) => {
    const next = Math.min(40, Math.max(0.4, view.zoom * factor));
    const f = next / view.zoom;
    const { cx, cy } = centre(host);
    // Keep the point under the pointer where it is.
    view.panX = (x - cx) * (1 - f) + view.panX * f;
    view.panY = (y - cy) * (1 - f) + view.panY * f;
    view.zoom = next;
    draw();
  };
  host.addEventListener(
    'wheel',
    (e) => {
      if (/** @type {HTMLElement} */ (e.target).closest('.plan-bar')) return;
      e.preventDefault();
      const { x, y } = local(e);
      zoomAt(x, y, Math.exp(-e.deltaY * (e.deltaMode === 1 ? 0.05 : 0.0015)));
    },
    { passive: false },
  );
  /** Where the press started, and how far a press may wander and still be a tap (a finger
   * wanders more than a mouse). */
  let start = { x: 0, y: 0 };
  let slop = 5;
  host.addEventListener('pointerdown', (e) => {
    if (/** @type {HTMLElement} */ (e.target).closest('button, select, .plan-bar')) return;
    pointers.set(e.pointerId, local(e));
    if (pointers.size === 1) {
      moved = 0;
      start = local(e);
      slop = e.pointerType === 'mouse' ? 5 : 12;
    }
    if (pointers.size === 2) {
      const [a, b] = [...pointers.values()];
      pinch = Math.hypot(a.x - b.x, a.y - b.y);
      moved = 100;
    }
  });
  host.addEventListener('pointermove', (e) => {
    const last = pointers.get(e.pointerId);
    if (!last) return;
    const now = local(e);
    if (pointers.size === 1) {
      // Not a tap once it has gone further than the slop from where it started; then it pans.
      moved = Math.max(moved, Math.hypot(now.x - start.x, now.y - start.y));
      if (moved > slop) {
        if (!host.hasPointerCapture(e.pointerId)) host.setPointerCapture(e.pointerId);
        view.panX += now.x - last.x;
        view.panY += now.y - last.y;
        draw();
      }
      pointers.set(e.pointerId, now);
      return;
    }
    pointers.set(e.pointerId, now);
    const [a, b] = [...pointers.values()];
    const d = Math.hypot(a.x - b.x, a.y - b.y);
    moved = 100;
    if (pinch > 0 && d > 0) zoomAt((a.x + b.x) / 2, (a.y + b.y) / 2, d / pinch);
    pinch = d;
  });
  const up = (e) => {
    pointers.delete(e.pointerId);
    if (pointers.size < 2) pinch = 0;
  };
  // Safari on an iPhone pinches with its own gesture events (and would zoom the page): the plan
  // zooms with them instead, when two-finger pointer events didn't already.
  let gesture = 1;
  host.addEventListener('gesturestart', (e) => {
    e.preventDefault();
    gesture = 1;
  });
  host.addEventListener('gesturechange', (e) => {
    e.preventDefault();
    const scale = Number(/** @type {any} */ (e).scale) || 1;
    if (pinch === 0 && scale > 0) {
      const { cx, cy } = centre(host);
      zoomAt(cx, cy, scale / gesture);
    }
    gesture = scale;
    moved = 100;
  });
  host.addEventListener('gestureend', (e) => e.preventDefault());
  host.addEventListener('pointerup', up);
  host.addEventListener('pointercancel', up);
  // A drag or a pinch is not a tap.
  host.addEventListener(
    'click',
    (e) => {
      if (moved > slop && !(/** @type {HTMLElement} */ (e.target).closest('button, select, .plan-bar'))) {
        e.stopPropagation();
        e.preventDefault();
      }
    },
    true,
  );
  host.addEventListener('click', (e) => {
    const t = /** @type {HTMLElement} */ (e.target);
    if (!t.closest('.plan-part[data-part], .plan-mark, .plan-add, .plan-bar, .plan-zoom')) drawn?.handlers.onSelect(null);
  });
  host.addEventListener('dblclick', (e) => {
    if (!(/** @type {HTMLElement} */ (e.target).closest('.plan-part, .plan-mark, .plan-add, .plan-bar, .plan-zoom'))) fitPlan();
  });
  host.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && drawn?.handlers.selected) drawn.handlers.onSelect(null);
  });
  new ResizeObserver(() => {
    if (drawn && !host.hidden) draw();
  }).observe(host);
}

/** The drawing styles a picture of the plan keeps (the page's CSS doesn't go with the SVG). */
const DRAWN = ['fill', 'fill-opacity', 'stroke', 'stroke-width', 'stroke-opacity', 'stroke-dasharray', 'stroke-linecap', 'stroke-linejoin', 'opacity', 'visibility', 'display', 'font-family', 'font-size', 'font-weight', 'text-anchor', 'dominant-baseline'];

/**
 * The plan as it's shown, as a PNG (a data URL), at `scale` times the screen: the parts where
 * they are (taken apart too), their names, the axis; not the controls (add buttons, zoom, the
 * bar) nor the marks.
 * @param {HTMLElement} host
 * @param {number} [scale]
 * @returns {Promise<string>}
 */
export async function capturePlan(host, scale = 2) {
  const [W, H] = [host.clientWidth, host.clientHeight];
  const canvas = document.createElement('canvas');
  canvas.width = Math.round(W * scale);
  canvas.height = Math.round(H * scale);
  const ctx = /** @type {CanvasRenderingContext2D} */ (canvas.getContext('2d'));
  ctx.fillStyle = getComputedStyle(host).backgroundColor;
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  const svg = host.querySelector(':scope > svg:not(.plan-stems)');
  if (svg) {
    const copy = /** @type {SVGSVGElement} */ (svg.cloneNode(true));
    // Each element's look, from the page's styles, written onto the copy.
    const from = [svg, ...svg.querySelectorAll('*')];
    const to = [copy, ...copy.querySelectorAll('*')];
    from.forEach((n, i) => {
      const style = getComputedStyle(n);
      const css = DRAWN.map((k) => `${k}:${style.getPropertyValue(k)}`).join(';');
      to[i].setAttribute('style', css);
    });
    for (const n of copy.querySelectorAll('.plan-mark, .plan-hit, .plan-halo, title')) n.remove();
    // Every part as plainly as the others, selected or not (one hidden from the 3D view stays
    // faint, as it's shown).
    for (const n of copy.querySelectorAll('.plan-part.dim:not(.off) .plan-wall')) /** @type {SVGElement} */ (n).style.opacity = '0.92';
    copy.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
    copy.setAttribute('width', String(W));
    copy.setAttribute('height', String(H));
    const img = new Image();
    img.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(new XMLSerializer().serializeToString(copy))}`;
    await img.decode();
    ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
  }
  return canvas.toDataURL('image/png');
}

/** Zooms about the middle of the plan (the zoom buttons). */
function zoomCentre(factor) {
  if (!drawn) return;
  const next = Math.min(40, Math.max(0.4, view.zoom * factor));
  const f = next / view.zoom;
  view.panX *= f;
  view.panY *= f;
  view.zoom = next;
  draw();
}

/** The middle of the host's drawing area (px), where zooming starts from. */
function centre(host) {
  return { cx: host.clientWidth / 2, cy: host.clientHeight / 2 };
}

/** What a control in the plan is, to find it again once the plan is drawn anew. */
function focusKey(n) {
  if (!(n instanceof Element)) return null;
  const set = n.getAttribute('data-set');
  const part = n.getAttribute('data-part');
  if (set) return `set:${set}`;
  if (part) return `part:${part}`;
  return `${n.getAttribute('class') || ''}|${n.getAttribute('aria-label') || ''}`;
}

/** Draws what renderPlan was last given, at the current zoom and pan (the focus kept). */
function draw() {
  if (!drawn) return;
  const { host } = drawn;
  const had = host.contains(document.activeElement) ? focusKey(document.activeElement) : null;
  paint();
  if (!had) return;
  // The same control, or (it's gone: the add button used) the selected part's bar.
  const again = [...host.querySelectorAll('[tabindex], button, select')].find((n) => focusKey(n) === had) || host.querySelector('.plan-bar select, .plan-bar button');
  /** @type {HTMLElement | null | undefined} */ (again)?.focus({ preventScroll: true });
}

function paint() {
  if (!drawn) return;
  const { host, result, handlers } = drawn;
  host.replaceChildren();
  const W = host.clientWidth;
  const H = host.clientHeight;
  if (W < 10 || H < 10) return;
  const hidden = handlers.hidden || new Set();
  const parts = sections(result);
  const partNames = new Set((result.partList || []).map((p) => p.name));
  const selected = handlers.selected && partNames.has(handlers.selected) ? handlers.selected : null;
  const component = (result.components || [])[0];
  const marks = (result.marks || []).filter((m) => {
    const part = markPart(m, partNames);
    return !part || !hidden.has(part);
  });
  // Taken apart (the assembly row): each part drawn where it has moved to; the ports have moved
  // with them, so no add buttons until it's together again.
  const offsets = handlers.offsets || new Map();
  const apart = [...offsets.values()].some((dz) => Math.abs(dz) > 1e-6);
  const free = component && !apart ? freePorts(result, parts, hidden) : [];
  // No part yet: one add button where the first one goes (on what its input is by default).
  const empty = component && !partNames.size;

  // The drawing's extent (mm): parts, marks and ports.
  let [rmax, zmin, zmax] = [1, Infinity, -Infinity];
  const take = (r, z) => {
    rmax = Math.max(rmax, Math.abs(r));
    zmin = Math.min(zmin, z);
    zmax = Math.max(zmax, z);
  };
  for (const [name, { points, width }] of parts) for (const [r, z] of points) take(r + width / 2, z + (offsets.get(name) || 0));
  for (const m of marks) for (const line of m.lines) for (const [x, , z] of line) take(x, z);
  for (const { port } of free) take(port.value[0], port.value[1]);
  if (!Number.isFinite(zmin)) [zmin, zmax, rmax] = [0, 100, 50];
  const inset = { ...INSET, bottom: INSET.bottom + (selected ? BAR : 0) };
  const k0 = Math.min((W - inset.left - inset.right) / (2 * rmax), (H - inset.top - inset.bottom) / Math.max(1, zmax - zmin));
  const k = Math.max(0.01, k0) * view.zoom;
  // Fitted: the drawing in the middle of the room left round it; then zoomed about the middle.
  const fx = inset.left + (W - inset.left - inset.right) / 2;
  const fy = inset.top + (H - inset.top - inset.bottom) / 2 + ((zmax + zmin) / 2) * k0;
  const { cx, cy } = centre(host);
  const X = (r) => cx + (fx - cx) * view.zoom + view.panX + r * k;
  const Y = (z) => cy + (fy - cy) * view.zoom + view.panY - z * k;

  const svg = document.createElementNS(SVG, 'svg');
  svg.setAttribute('viewBox', `0 0 ${W} ${H}`);
  svg.setAttribute('role', 'group');
  svg.setAttribute('aria-label', 'Plan');
  const node = (name, attrs, parent = svg) => {
    const n = document.createElementNS(SVG, name);
    for (const [key, v] of Object.entries(attrs)) n.setAttribute(key, String(v));
    parent.append(n);
    return n;
  };
  const line = (pts) => pts.map(([x, y]) => `${x.toFixed(1)},${y.toFixed(1)}`).join(' ');
  node('line', { x1: X(0), y1: 0, x2: X(0), y2: H, class: 'plan-axis' });

  // The parts, the selected one with a halo; the others a little fainter while one is selected.
  const hit = collidingParts(result.diagnostics);
  [...parts.entries()].forEach(([name, { points, width }], i) => {
    const off = hidden.has(name);
    const cls = ['plan-part', hit.has(name) && 'collides', selected === name && 'selected', selected && selected !== name && 'dim', off && 'off'].filter(Boolean).join(' ');
    // A part (`part` line) is selected by a tap; a recipe's only toolpath is just drawn.
    const own = partNames.has(name);
    const g = node('g', own ? { class: cls, 'data-part': name, tabindex: 0, role: 'button', 'aria-pressed': String(selected === name), 'aria-label': name } : { class: `${cls} still` });
    if (offsets.get(name)) g.setAttribute('transform', `translate(0 ${(-(offsets.get(name) || 0) * k).toFixed(1)})`);
    g.style.setProperty('--part', handlers.colorOf?.(name) ?? COLORS[i % COLORS.length]);
    const px = Math.max(1.5, width * k);
    const halves = [1, -1].map((side) => line(points.map(([r, z]) => [X(side * r), Y(z)])));
    if (selected === name) for (const pts of halves) node('polyline', { points: pts, 'stroke-width': px + 8, class: 'plan-halo' }, g);
    for (const pts of halves) node('polyline', { points: pts, 'stroke-width': Math.max(14, px), class: 'plan-hit' }, g);
    for (const pts of halves) node('polyline', { points: pts, 'stroke-width': px, class: 'plan-wall' }, g);
    // Its name on the left, beside its widest point (the right is for the add buttons).
    // (Where it's as wide for a while, as a tube is all along, half way along that.)
    const widest = Math.max(...points.map((p) => p[0]));
    const there = points.filter((p) => p[0] >= widest - 0.5).sort((a, b) => a[1] - b[1]);
    const far = there[Math.floor(there.length / 2)];
    if (far && own) {
      const label = node('text', { x: Math.max(4, X(-far[0]) - px / 2 - 6), y: Y(far[1]), class: 'plan-label', 'text-anchor': X(-far[0]) - px / 2 - 6 < 40 ? 'start' : 'end' }, g);
      label.textContent = name;
    }
    if (!own) return;
    const pick = () => handlers.onSelect(selected === name ? null : name);
    g.addEventListener('click', pick);
    g.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        pick();
      }
    });
  });

  // The marks: each piece of the shape that leads to its settings, a band to tap (over the parts).
  for (const mark of marks) {
    const part = markPart(mark, partNames);
    const what = mark.label || mark.settings[0];
    const g = node('g', { class: `plan-mark${selected && part !== selected ? ' dim' : ''}`, tabindex: 0, role: 'button', 'aria-label': part ? `${what} · ${part}` : what });
    if (part && offsets.get(part)) g.setAttribute('transform', `translate(0 ${(-(offsets.get(part) || 0) * k).toFixed(1)})`);
    node('title', {}, g).textContent = part ? `${what} · ${part}` : what;
    for (const ln of mark.lines) {
      const pts = ln.map(([x, , z]) => [X(x), Y(z)]);
      if (pts.length === 1) pts.push([pts[0][0] + 0.01, pts[0][1]]);
      node('polyline', { points: line(pts), class: 'plan-mark-hit' }, g);
      node('polyline', { points: line(pts), class: 'plan-mark-line' }, g);
    }
    const go = () => handlers.onMark(mark, part);
    g.addEventListener('click', go);
    g.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        go();
      }
    });
  }
  host.append(svg);

  // The add buttons: a pair (up, down) beside every free port of the selected part (of the only
  // part, when there's one), on the face's side of the wall (outside to the right of it, inside
  // to the left), a stem to where it screws on. The other parts' free ports are dots.
  const active = selected ?? (partNames.size === 1 ? [...partNames][0] : null);
  const dirAsk = askOf(component, 'direction');
  const ways = dirAsk ? dirAsk.choices.map(String) : [''];
  /** @type {{ port: any, along: number, ways: string[], ax: number, ay: number, x: number, y: number, w: number, h: number, side: number }[]} */
  const pills = [];
  /** @type {number[][]} */
  const dots = [];
  for (const { port, along } of free) {
    const [r, z, inward, , , faceSide] = port.value;
    const side = faceSide < 0 ? -1 : 1;
    // Where the straight start would sit: along the section from its end.
    const za = z + inward * along;
    const ax = X(r);
    const ay = Y(za + inward * Math.min(4, port.value[3] / 4));
    if (port.part !== active) {
      dots.push([ax, ay]);
      continue;
    }
    const h = ways.length * BUTTON + (ways.length - 1) * 2;
    pills.push({ port, along, ways, ax, ay, x: ax + side * (14 + BUTTON / 2), y: ay, w: BUTTON, h, side });
  }
  if (empty) {
    const ways0 = dirAsk ? [String(dirAsk.value)] : [''];
    pills.push({ port: null, along: 0, ways: ways0, ax: X(0), ay: Y(0), x: X(0) + 14 + BUTTON / 2, y: Y(0) - BUTTON / 2, w: BUTTON, h: BUTTON, side: 1 });
  }
  spread(pills, W, H - (selected ? BAR : 0));
  if (pills.length || dots.length) {
    const stems = document.createElementNS(SVG, 'svg');
    stems.setAttribute('class', 'plan-stems');
    stems.setAttribute('viewBox', `0 0 ${W} ${H}`);
    stems.setAttribute('aria-hidden', 'true');
    for (const [x, y] of dots) node('circle', { cx: x, cy: y, r: 3, class: 'plan-dot free' }, stems);
    for (const p of pills) {
      if (!p.port) continue;
      const ex = p.x - p.side * (p.w / 2);
      node('line', { x1: p.ax, y1: p.ay, x2: ex, y2: p.y, class: 'plan-stem' }, stems);
      node('circle', { cx: p.ax, cy: p.ay, r: 3.5, class: 'plan-dot' }, stems);
    }
    host.append(stems);
  }
  for (const p of pills) {
    const where = p.port ? `${p.port.part} · ${p.port.label}` : 'first';
    const group = el('div', { className: `plan-add${p.side < 0 ? ' inside' : ''}`, role: 'group', 'aria-label': `Add ${String(component?.label || 'part').toLowerCase()}: ${where}` });
    group.style.left = `${p.x - p.w / 2}px`;
    group.style.top = `${p.y - p.h / 2}px`;
    for (const way of p.ways) {
      const label = `Add ${String(component?.label || 'part').toLowerCase()}: ${where}${way ? `, going ${way}` : ''}`;
      const b = el('button', { type: 'button', className: 'plan-add-btn', 'aria-label': label, title: label, 'data-way': way, 'data-port': p.port?.name ?? '' });
      b.append(addIcon(way));
      b.addEventListener('click', () => {
        /** @type {Record<string, unknown>} */
        const values = {};
        if (way) values.direction = way;
        if (p.along > 0) values.along = p.along;
        handlers.onAdd({ port: p.port, values });
      });
      group.append(b);
    }
    host.append(group);
  }

  // Zoom buttons (a sure way on a phone, where a pinch may belong to the page).
  const zoom = el('div', { className: 'plan-zoom', role: 'group', 'aria-label': 'Zoom' });
  for (const [label, f] of /** @type {[string, number][]} */ ([['+', 1.5], ['−', 1 / 1.5]])) {
    const b = el('button', { type: 'button', text: label, 'aria-label': f > 1 ? 'Zoom in' : 'Zoom out', title: f > 1 ? 'Zoom in' : 'Zoom out' });
    b.addEventListener('click', () => zoomCentre(f));
    zoom.append(b);
  }
  host.append(zoom);

  if (selected) host.append(partBar(result, selected, component, handlers));
}

/**
 * Moves the add buttons apart where they'd overlap (each other, or off the plan): along the
 * wall, up or down, as little as it takes.
 * @param {{ x: number, y: number, w: number, h: number }[]} pills
 * @param {number} W
 * @param {number} H
 */
function spread(pills, W, H) {
  const gap = 6;
  for (let round = 0; round < 40; round++) {
    let moved = false;
    for (let i = 0; i < pills.length; i++) {
      for (let j = i + 1; j < pills.length; j++) {
        const [a, b] = [pills[i], pills[j]];
        const ox = (a.w + b.w) / 2 + gap - Math.abs(a.x - b.x);
        const oy = (a.h + b.h) / 2 + gap - Math.abs(a.y - b.y);
        if (ox <= 0 || oy <= 0) continue;
        // Up and down, the higher one up.
        const s = a.y < b.y || (a.y === b.y && i < j) ? -1 : 1;
        a.y += (s * oy) / 2;
        b.y -= (s * oy) / 2;
        moved = true;
      }
    }
    for (const p of pills) {
      const y = Math.min(H - p.h / 2 - 4, Math.max(p.h / 2 + 4, p.y));
      const x = Math.min(W - p.w / 2 - 4, Math.max(p.w / 2 + 4, p.x));
      if (x !== p.x || y !== p.y) moved = true;
      p.x = x;
      p.y = y;
    }
    if (!moved) break;
  }
}

/** A `+` with an arrow the way a new part goes (up, down; none: just the `+`). */
function addIcon(way) {
  const svg = document.createElementNS(SVG, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('aria-hidden', 'true');
  const path = document.createElementNS(SVG, 'path');
  const plus = way ? 'M4 12h8M8 8v8' : 'M6 12h12M12 6v12';
  const arrow = way === 'up' ? 'M17.5 20V5M13.5 9l4-4 4 4' : way === 'down' ? 'M17.5 4v15M13.5 15l4 4 4-4' : '';
  path.setAttribute('d', `${plus}${arrow}`);
  svg.append(path);
  return svg;
}

/** A small line icon for the bar's buttons. */
function icon(d) {
  const svg = document.createElementNS(SVG, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('aria-hidden', 'true');
  const path = document.createElementNS(SVG, 'path');
  path.setAttribute('d', d);
  svg.append(path);
  return svg;
}

const ICONS = {
  settings: 'M4 7h10M18 7h2M4 17h4M12 17h8M16 5v4M10 15v4',
  remove: 'M5 7h14M10 7V5h4v2M7 7l1 13h8l1-13M10.5 11v6M13.5 11v6',
  close: 'M6 6l12 12M18 6L6 18',
  up: 'M12 20V5M7 10l5-5 5 5',
  down: 'M12 4v15M7 14l5 5 5-5',
};

/** A setting's label without what's in brackets ("Straight end (more shades…)": "Straight end"). */
const shortLabel = (label) => String(label).replace(/\s*\(.*\)\s*$/, '').trim();

/**
 * The selected part's bar: its name; its `@ask` settings (a list for a choice of many, a flip for
 * a choice of two, up and down as arrows); its on/off settings that apply; its settings; hide in
 * the 3D view; remove; close.
 */
function partBar(result, name, component, handlers) {
  const params = result.params.filter((p) => p.part === name);
  const values = Object.fromEntries(result.params.map((p) => [p.name, p.value]));
  const own = (p) => p.name.slice(name.length + 1);
  const bar = el('div', { className: 'plan-bar', role: 'toolbar', 'aria-label': name });
  const color = handlers.colorOf?.(name);
  const title = el('button', { type: 'button', className: 'plan-bar-name', text: name, title: `${name}: settings`, 'aria-label': `${name}: settings` });
  if (color) title.style.setProperty('--part', color);
  title.addEventListener('click', () => handlers.onOpen(name));
  bar.append(title);
  const tool = (cls, d, label, go) => {
    const b = el('button', { type: 'button', className: `plan-bar-btn ${cls}`, title: label, 'aria-label': label });
    b.append(icon(d));
    b.addEventListener('click', go);
    return b;
  };
  // The `@ask` settings, a list (the shape) before a flip (up, down).
  const asked = new Set((component?.ask || []).map((p) => p.name));
  const askedHere = params.filter((p) => asked.has(own(p)) && p.kind === 'choice').sort((a, b) => b.choices.length - a.choices.length);
  for (const p of askedHere) {
    const key = own(p);
    const value = String(p.value);
    if (p.choices.length === 2) {
      const next = String(p.choices.find((c) => String(c) !== value));
      const arrows = value === 'up' || value === 'down';
      const b = el('button', { type: 'button', className: 'plan-bar-btn', 'data-set': key, title: `${p.label}: ${value} → ${next}`, 'aria-label': `${p.label}: ${value}, switch to ${next}` });
      if (arrows) b.append(icon(ICONS[/** @type {'up' | 'down'} */ (value)]));
      else b.textContent = value;
      b.addEventListener('click', () => handlers.onSet(name, { [key]: next }));
      bar.append(b);
    } else {
      const select = /** @type {HTMLSelectElement} */ (el('select', { className: 'plan-bar-select', 'data-set': key, 'aria-label': p.label, title: p.label }, p.choices.map((c) => el('option', { value: String(c), text: String(c) }))));
      select.value = value;
      select.addEventListener('change', () => handlers.onSet(name, { [key]: select.value }));
      bar.append(select);
    }
  }
  bar.append(tool('settings', ICONS.settings, `${name}: settings`, () => handlers.onOpen(name)), tool('remove', ICONS.remove, `Remove ${name}`, () => handlers.onRemove(name)));
  // Its on/off settings that apply (a straight end, …).
  for (const p of params) {
    if (p.kind !== 'toggle' || p.attrs.includes('hidden') || !applies(p, result.params, values)) continue;
    const on = p.value === true;
    const b = el('button', { type: 'button', className: 'plan-bar-chip', 'aria-pressed': String(on), 'data-set': own(p), text: shortLabel(p.label), title: p.label });
    b.addEventListener('click', () => handlers.onSet(name, { [own(p)]: !on }));
    bar.append(b);
  }
  bar.append(tool('close', ICONS.close, 'Done', () => handlers.onSelect(null)));
  return bar;
}

/**
 * A free name for a new part, after its shape: `tube`, `tube2`, …
 * @param {string} shape
 * @param {string[]} taken
 */
export function newPartName(shape, taken) {
  const base = String(shape || 'part').toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_+|_+$/g, '') || 'part';
  const start = /^[a-z_]/.test(base) ? base : `part_${base}`;
  let name = start;
  for (let k = 2; taken.includes(name); k++) name = `${start}${k}`;
  return name;
}

/**
 * Sensible starting values for a new shade on a port, scaled to where it goes (the template's
 * own defaults are for a shade on an E27 socket, 23 mm round): outside, it opens out; inside, it
 * stays within. And which way it goes: on from the port's section (away from what it's on).
 * @param {string} shape
 * @param {number[]} port [radius, height, inward, length, socket, side (1 outside, -1 inside)]
 */
export function shadeDefaults(shape, port) {
  const r = Math.max(5, port[0]);
  const inside = port[5] < 0;
  const d = (v) => Math.max(20, Math.round(v));
  /** @type {Record<string, unknown>} */
  const values = { direction: port[2] < 0 ? 'up' : 'down' };
  // Outside it opens out (as the template's shade does from the socket); inside, it narrows.
  const out = (extra) => d(inside ? 2 * r - extra / 2 : 2 * r + extra);
  if (['cone', 'bell', 'dome', 'own'].includes(shape)) values.opening = out(104);
  if (['sphere', 'flat sphere'].includes(shape)) {
    values.sphere = d(2 * r + (inside ? 20 : 154));
    values.opening = out(104);
  }
  if (['bell in-out', 'cone in-out'].includes(shape)) values.widest = inside ? d(2 * r) : out(154);
  if (shape === 'own') values.own_r = d(inside ? r - 10 : r + 27);
  return values;
}

/**
 * The values a part's shape brings when it's switched to another: the new shape's sizes scaled to
 * where the part sits, for the settings the old shape didn't use (those it did keep their values).
 * @param {string} from the old shape
 * @param {string} to the new shape
 * @param {number[] | null} port the port it's on (null: the default, e.g. the socket)
 */
export function shapeSwitch(from, to, port) {
  if (!port) return {};
  const before = shadeDefaults(from, port);
  const after = shadeDefaults(to, port);
  /** @type {Record<string, unknown>} */
  const out = {};
  for (const [k, v] of Object.entries(after)) if (k !== 'direction' && !(k in before)) out[k] = v;
  return out;
}
