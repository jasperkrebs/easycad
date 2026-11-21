// The lamp's fittings, for the view, the plan and the render: a standard lampholder (E27 or
// E14) where the shade screws on, a standard frosted bulb in it (A60 pear on E27, P45 round on
// E14) and a cable going straight up. The socket always points down: the bulb hangs below it and
// the cable rises from its top. The project hangs either way round it (its shades going down
// past the bulb, or up), a view setting only: nothing in the recipe changes.
//
// Pure (no three.js, no DOM), so the node tests can check it. All sizes in mm.

/**
 * Lampholders and bulbs by socket. Holder: the shade thread's diameter (the body), the body's
 * height; the cable gland on top. Bulb (IEC 60064 shapes): the cap's diameter, the glass's
 * widest diameter and its length below the cap (overall length less the cap, A60 E27 about
 * 108 mm, P45 E14 about 80 mm).
 */
export const FITTINGS = {
  E27: { body: 40, height: 55, gland: 13, glandH: 10, bulb: 'A60', cap: 26.5, glass: 60, glassL: 83, shoulder: 45 },
  E14: { body: 28, height: 45, gland: 11, glandH: 8, bulb: 'P45', cap: 14, glass: 45, glassL: 58, shoulder: 30 },
};
/** The cable's diameter, and the cap's metal showing between the holder and the glass. */
export const CABLE_D = 6;
export const CAP_SHOWS = 3;

/**
 * The socket a recipe is built round, from its settings: `socket` (E27, E14 or custom: then its
 * diameter from `thread_d` or `socket_d`, and height from `socket_h`). null when the recipe has
 * no socket.
 * @param {{ name: string, value: unknown }[]} params
 */
export function socketOf(params) {
  const get = (name) => (params || []).find((p) => p.name === name)?.value;
  const kind = String(get('socket') ?? '');
  if (kind === 'E27' || kind === 'E14') return { kind, ...FITTINGS[kind] };
  if (kind !== 'custom') return null;
  const d = Number(get('thread_d') ?? get('socket_d'));
  const h = Number(get('socket_h'));
  const base = FITTINGS[d > 0 && d < 34 ? 'E14' : 'E27'];
  return { kind, ...base, body: d > 0 ? d : base.body, height: h > 0 ? h : base.height };
}

/**
 * Where the socket goes: on the straight section that screws onto it, the part whose beads run
 * round the holder's body (within a bead of it). Its socket face (where the bulb comes out) is
 * at the section's lower end as the lamp hangs, the body rising from there. The lamp hangs with
 * its shades down past the bulb (`hang` 'down', the default) or up away from it: `flipped` says
 * whether the project is turned over for that (its +z then points down).
 * @param {{ name: string, value: unknown }[]} params the recipe's settings
 * @param {Map<string, { points: number[][], width: number }>} parts each part's (radius, height)
 *   points and bead width, as the plan has them (parts.js `sections`)
 * @param {'down' | 'up'} [hang]
 * @returns {null | ReturnType<typeof socketOf> & { face: number, up: number, flipped: boolean, part: string | null }}
 *   `face`: the socket face's height in the recipe's z; `up`: which way the body rises in the
 *   recipe's z (+1 or -1)
 */
export function fixtureOf(params, parts, hang = 'down') {
  const socket = socketOf(params);
  if (!socket) return null;
  const r0 = socket.body / 2;
  // The part with the most points round the body, and those points' heights.
  let best = { part: /** @type {string | null} */ (null), zs: /** @type {number[]} */ ([]), rest: /** @type {number[]} */ ([]) };
  for (const [name, { points, width }] of parts || new Map()) {
    const lo = r0 - 2;
    const hi = r0 + 0.75 * Math.max(0.4, width) + 1;
    const zs = [];
    const rest = [];
    for (const [r, z] of points) (r >= lo && r <= hi ? zs : rest).push(z);
    if (zs.length > best.zs.length) best = { part: name, zs, rest };
  }
  let [a, b] = [0, 0];
  let shadesUp = true;
  if (best.zs.length) {
    a = Math.min(...best.zs);
    b = Math.max(...best.zs);
    // The shade leaves the section on the side where the rest of the part is.
    const mid = (a + b) / 2;
    const mean = best.rest.length ? best.rest.reduce((s, z) => s + z, 0) / best.rest.length : mid + 1;
    shadesUp = mean >= mid;
  }
  // Hanging down: the shades below the socket face. Turned over, the recipe's +z points down.
  const flipped = hang === 'up' ? !shadesUp : shadesUp;
  return { ...socket, face: flipped ? b : a, up: flipped ? -1 : 1, flipped, part: best.part };
}

/** A smooth curve from p0 to p1 leaving along t0 and arriving along t1 (cubic Hermite). */
function hermite(p0, t0, p1, t1, n) {
  const out = [];
  for (let i = 1; i <= n; i++) {
    const s = i / n;
    const [h00, h10, h01, h11] = [2 * s ** 3 - 3 * s ** 2 + 1, s ** 3 - 2 * s ** 2 + s, -2 * s ** 3 + 3 * s ** 2, s ** 3 - s ** 2];
    out.push([0, 1].map((k) => h00 * p0[k] + h10 * t0[k] + h01 * p1[k] + h11 * t1[k]));
  }
  return out;
}

/**
 * The bulb's glass as [radius, depth below the glass's top] from the neck round to the tip (a
 * closed lathe profile: it starts and ends on the axis). The pear (A60) flares from its neck into
 * the sphere; the round (P45) is a sphere on a short neck.
 * @param {ReturnType<typeof socketOf>} f
 */
export function bulbProfile(f) {
  const neck = f.cap / 2;
  const R = f.glass / 2;
  const c = f.glassL - R;
  const phi0 = (f.shoulder * Math.PI) / 180;
  const shoulder = [R * Math.sin(phi0), c - R * Math.cos(phi0)];
  const reach = Math.hypot(shoulder[0] - neck, shoulder[1]);
  const pts = [[0, 0], [neck, 0]];
  pts.push(...hermite([neck, 0], [0, reach * 0.9], shoulder, [Math.cos(phi0) * reach * 0.9, Math.sin(phi0) * reach * 0.9], 14));
  const steps = 28;
  for (let i = 1; i <= steps; i++) {
    const phi = phi0 + ((Math.PI - phi0) * i) / steps;
    pts.push([Math.max(0, R * Math.sin(phi)), c - R * Math.cos(phi)]);
  }
  pts[pts.length - 1][0] = 0;
  return pts;
}

/** Where the light is: the glass's sphere's centre, below the glass's top. */
export function bulbCentre(f) {
  return f.glassL - f.glass / 2;
}

/**
 * The holder as a closed lathe profile [radius, height above the socket face]: the body (its
 * edges softened), then the cable gland on top.
 * @param {ReturnType<typeof socketOf>} f
 */
export function socketProfile(f) {
  const r = f.body / 2;
  const g = f.gland / 2;
  const e = Math.min(2, r / 6);
  return [[0, 0], [r - e, 0], [r, e], [r, f.height - e], [r - e, f.height], [g, f.height], [g, f.height + f.glandH], [0, f.height + f.glandH]];
}

/**
 * The fittings drawn in the plan, in the recipe's (radius, z): closed outlines (holder, cap,
 * bulb) on both sides of the axis, and the cable's two edges going up `cable` mm.
 * @param {NonNullable<ReturnType<typeof fixtureOf>>} fx
 * @param {number} [cable]
 * @returns {{ socket: number[][], cap: number[][], bulb: number[][], cable: number[][][] }}
 */
export function fixtureOutline(fx, cable = 1000) {
  const z = (s) => fx.face + fx.up * s;
  const both = (profile, at) => {
    const right = profile.map(([r, s]) => [r, z(at(s))]);
    const left = profile.map(([r, s]) => [-r, z(at(s))]).reverse();
    return [...right, ...left];
  };
  const top = fx.height + fx.glandH;
  const neck = fx.cap / 2;
  return {
    socket: both(socketProfile(fx), (s) => s),
    cap: both([[0, 0], [neck, 0], [neck, CAP_SHOWS], [0, CAP_SHOWS]], (s) => -s),
    bulb: both(bulbProfile(fx), (s) => -(CAP_SHOWS + s)),
    cable: [-1, 1].map((side) => [[(side * CABLE_D) / 2, z(top)], [(side * CABLE_D) / 2, z(top + cable)]]),
  };
}

/**
 * A light's colour at a colour temperature (kelvin), as linear-ish RGB 0..1 (the brightest
 * channel 1): Tanner Helland's fit to the black-body colours, good from 1000 K to 40000 K.
 * @param {number} kelvin
 * @returns {[number, number, number]}
 */
export function kelvinColor(kelvin) {
  const t = Math.min(400, Math.max(10, kelvin / 100));
  const clamp = (v) => Math.min(255, Math.max(0, v)) / 255;
  const r = t <= 66 ? 255 : 329.698727446 * (t - 60) ** -0.1332047592;
  const g = t <= 66 ? 99.4708025861 * Math.log(t) - 161.1195681661 : 288.1221695283 * (t - 60) ** -0.0755148492;
  const b = t >= 66 ? 255 : t <= 19 ? 0 : 138.5177312231 * Math.log(t - 10) - 305.0447927307;
  const rgb = [clamp(r), clamp(g), clamp(b)];
  const top = Math.max(...rgb) || 1;
  return /** @type {[number, number, number]} */ (rgb.map((v) => v / top));
}

/** The render's settings and their defaults (kept per project with the custom colours). */
export const RENDER_DEFAULTS = {
  /** The bulb on (its light the main one) or off (daylight round the lamp). */
  light: true,
  /** The bulb's light output in lumens (806: a 60 W bulb's) and its colour temperature. */
  lumens: 806,
  kelvin: 2700,
  /** The light round the lamp while the bulb is on (1: as with it off). */
  room: 0.06,
  /** Exposure, ±stops (a shade lit from inside shows its glow a stop up). */
  exposure: 1,
  /** How rough (matte) the printed surface is, 0 glossy to 1 matte. */
  matte: 0.35,
  /** Samples per pixel to stop at, light bounces, and pixels per screen pixel. */
  samples: 256,
  bounces: 8,
  scale: 1,
};

/**
 * Render settings from anything (a file, storage), every value checked and within its range;
 * what's missing or wrong takes its default.
 * @param {any} raw
 * @returns {typeof RENDER_DEFAULTS}
 */
export function cleanRender(raw) {
  const r = raw && typeof raw === 'object' ? raw : {};
  const num = (k, lo, hi) => {
    const v = Number(r[k]);
    return Number.isFinite(v) ? Math.min(hi, Math.max(lo, v)) : RENDER_DEFAULTS[k];
  };
  return {
    light: typeof r.light === 'boolean' ? r.light : RENDER_DEFAULTS.light,
    lumens: num('lumens', 50, 5000),
    kelvin: num('kelvin', 1800, 6500),
    room: num('room', 0, 1),
    exposure: num('exposure', -6, 6),
    matte: num('matte', 0, 1),
    samples: Math.round(num('samples', 1, 10000)),
    bounces: Math.round(num('bounces', 1, 32)),
    scale: num('scale', 0.25, 4),
  };
}
