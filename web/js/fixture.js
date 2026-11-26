// The lamp's fittings, for the view, the plan and the render: a standard lampholder (E27 or
// E14) where the shade screws on, a standard frosted bulb in it (A60 pear on E27, P45 round on
// E14) and a cable going straight up. The socket always points down: the bulb hangs below it and
// the cable rises from its top. The project hangs either way round it (its shades going down
// past the bulb, or up), a view setting only: nothing in the recipe changes.
//
// Pure (no three.js, no DOM), so the node tests can check it. All sizes in mm.

/**
 * Lampholders and bulbs by socket. Holder: the shade thread's diameter (the body: the thread's
 * crests) and pitch (the recipes' own: E27 2.5 mm, E14 2 mm), the body's height; the cable gland
 * on top. Bulb (IEC 60064 shapes): the cap's diameter, the glass's
 * widest diameter and its length below the cap (overall length less the cap, A60 E27 about
 * 108 mm, P45 E14 about 80 mm).
 */
export const FITTINGS = {
  E27: { capPitch: 3.629, capThread: 20, capDepth: 1.1, body: 40, pitch: 2.5, height: 55, gland: 13, glandH: 10, bulb: 'A60', cap: 26.5, glass: 60, glassL: 83, shoulder: 45 },
  E14: { capPitch: 2.822, capThread: 15, capDepth: 0.8, body: 28, pitch: 2, height: 45, gland: 11, glandH: 8, bulb: 'P45', cap: 14, glass: 45, glassL: 58, shoulder: 30 },
};
/**
 * The bulb's Edison screw cap (IEC 60061-1): its thread's pitch (E27 7 turns an inch, E14 9),
 * the threaded length and the thread's depth; the cap's diameter is `cap` (the thread's crests).
 * Screwed in, all of it but `CAP_SHOWS` is inside the holder.
 */
/** The cable's diameter, and the cap's metal showing between the holder and the glass. */
export const CABLE_D = 6;
export const CAP_SHOWS = 3;

/**
 * The socket a recipe is built round, from its settings: `socket` (E27, E14 or custom: then its
 * diameter from `thread_d` or `socket_d`, pitch from `thread_pitch` or `socket_pitch`, and height
 * from `socket_h`). null when the recipe has no socket.
 * @param {{ name: string, value: unknown }[]} params
 */
export function socketOf(params) {
  const get = (name) => (params || []).find((p) => p.name === name)?.value;
  const kind = String(get('socket') ?? '');
  if (kind === 'E27' || kind === 'E14') return { kind, ...FITTINGS[kind] };
  if (kind !== 'custom') return null;
  const d = Number(get('thread_d') ?? get('socket_d'));
  const h = Number(get('socket_h'));
  const pitch = Number(get('thread_pitch') ?? get('socket_pitch'));
  const base = FITTINGS[d > 0 && d < 34 ? 'E14' : 'E27'];
  return { kind, ...base, body: d > 0 ? d : base.body, pitch: pitch > 0 ? pitch : base.pitch, height: h > 0 ? h : base.height };
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
  return { ...socket, face: flipped ? b : a, up: flipped ? -1 : 1, flipped, part: best.part, throat: b - a, shadesBelow: hang !== 'up' };
}

/**
 * The fittings taken apart with the lamp (its exploded view), `u` of the way (0: in, 1: out), so
 * that, top to bottom as it hangs, it reads socket, shades, bulb: the socket goes up along the
 * cable until its face is a gap above the highest shade, the bulb unscrews from it and goes down
 * until the top of its screw cap (the whole bulb) is a gap below the lowest. `extent`: the shades' lowest and highest z in the
 * recipe's frame, where the exploded view has them. Adds `bulbDrop`: how far (mm) the bulb's cap
 * is below the socket's face.
 * @param {NonNullable<ReturnType<typeof fixtureOf>>} fx
 * @param {number} u
 * @param {[number, number] | null} [extent]
 */
export function fixtureTakenOut(fx, u, extent = null) {
  const t = Math.min(1, Math.max(0, u));
  if (!t || !extent) return { ...fx, bulbDrop: 0 };
  const gap = 20;
  // Heights along the cable (up as the lamp hangs): the recipe's z times `up`.
  const [h0, h1] = [extent[0] * fx.up, extent[1] * fx.up];
  const [lo, hi] = [Math.min(h0, h1), Math.max(h0, h1)];
  const face = fx.face * fx.up;
  // Where they end up (the whole socket above the shades, the whole bulb below: its cap a gap
  // under the lowest), never further in than they start.
  const faceOut = Math.max(face, hi + gap);
  // The cap reaches this far above the socket's face screwed in (its thread and contact): its top
  // goes the gap below.
  const capAbove = capTop(fx).contact[2][1];
  const capOut = Math.min(face, lo - gap - capAbove);
  const faceNow = face + (faceOut - face) * t;
  const capNow = face + (capOut - face) * t;
  return { ...fx, face: faceNow * fx.up, bulbDrop: faceNow - capNow };
}

/** The lowest and highest z of the parts' points (by name: [radius, z] lists), each moved by its offset. */
export function partsExtent(parts, offsets = new Map()) {
  let [lo, hi] = [Infinity, -Infinity];
  for (const [name, { points }] of parts) {
    const dz = offsets.get(name) || 0;
    for (const [, z] of points) {
      lo = Math.min(lo, z + dz);
      hi = Math.max(hi, z + dz);
    }
  }
  return Number.isFinite(lo) ? /** @type {[number, number]} */ ([lo, hi]) : null;
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
  const t = socketThread(f);
  // Under the thread the body is at the thread's root (the thread's own surface goes over it).
  const body = t ? [[r, t.z0], [t.root, t.z0], [t.root, t.z1], [r, t.z1]] : [];
  return [[0, 0], [r - e, 0], [r, e], ...body, [r, f.height - e], [r - e, f.height], [g, f.height], [g, f.height + f.glandH], [0, f.height + f.glandH]];
}

/**
 * The thread on the socket's body that the shade screws onto: all along the body (from its
 * bottom edge's bevel to its top's), its crests at the body's diameter (where the shade's bead runs),
 * its roots `depth` in (about half a pitch), right-handed, one pitch a turn. null when the body
 * is too short for a turn.
 * @param {ReturnType<typeof socketOf>} f
 */
export function socketThread(f) {
  const pitch = Number(f.pitch);
  if (!(pitch > 0)) return null;
  const r = f.body / 2;
  const depth = Math.min(0.45 * pitch, r / 8);
  // The whole body, edge to edge: a shade's straight start can sit anywhere along it.
  const edge = Math.min(2, r / 6);
  const z0 = edge;
  const z1 = f.height - edge;
  if (z1 - z0 < 2 * pitch) return null;
  return { pitch, depth, z0, z1, crest: r, root: r - depth };
}

/**
 * The thread's surface radius at angle `theta` (radians, counter-clockwise from above) and height
 * `z` above the face: a rounded tooth each pitch, rising one pitch a turn (right-handed), fading
 * in over the first half turn and out over the last, to the root, where it meets the body.
 * @param {NonNullable<ReturnType<typeof socketThread>>} t
 * @param {number} theta
 * @param {number} z
 */
export function threadRadius(t, theta, z) {
  const u = (z - t.z0) / t.pitch - theta / (2 * Math.PI);
  const tooth = 0.5 + 0.5 * Math.cos(2 * Math.PI * u);
  const fade = Math.min(1, Math.max(0, (2 * (z - t.z0)) / t.pitch), Math.max(0, (2 * (t.z1 - z)) / t.pitch));
  return t.root + t.depth * tooth * fade;
}

/**
 * The bulb's screw thread, in the fittings' frame (z up from the socket's face, the bulb screwed
 * in): from the glass's top (`CAP_SHOWS` below the face) up the threaded length, the crests at the
 * cap's diameter. The same shape as the socket's thread (`threadRadius` takes it).
 * @param {ReturnType<typeof socketOf>} f
 */
export function capThread(f) {
  const pitch = Number(f.capPitch) || 3.629;
  const depth = Number(f.capDepth) || 1.1;
  const r = f.cap / 2;
  return { pitch, depth, z0: -CAP_SHOWS, z1: (Number(f.capThread) || 20) - CAP_SHOWS, crest: r, root: r - depth };
}

/**
 * The cap above its thread: the black insulator and the brass contact at its middle, as
 * [radius, z] in the fittings' frame, the insulator's and the contact's lathe profiles.
 * @param {ReturnType<typeof socketOf>} f
 */
export function capTop(f) {
  const t = capThread(f);
  const ins = t.root * 0.72;
  const contact = Math.max(1.5, t.root * 0.22);
  return {
    insulator: [[0, t.z1], [ins, t.z1], [ins, t.z1 + 2.5], [0, t.z1 + 2.5]],
    contact: [[0, t.z1 + 2.5], [contact, t.z1 + 2.5], [contact * 0.8, t.z1 + 3.5], [0, t.z1 + 3.5]],
  };
}

/**
 * The cap cut through its axis on one side (angle `theta`): its thread's teeth from the glass up,
 * then the insulator and the contact, [radius, z] from the axis round to the axis.
 * @param {ReturnType<typeof socketOf>} f
 * @param {number} [theta]
 */
export function capSection(f, theta = 0) {
  const t = capThread(f);
  const top = capTop(f);
  const steps = Math.max(2, Math.ceil(((t.z1 - t.z0) / t.pitch) * 8));
  const teeth = [];
  for (let k = 0; k <= steps; k++) {
    const z = t.z0 + ((t.z1 - t.z0) * k) / steps;
    teeth.push([threadRadius(t, theta, z), z]);
  }
  return [[0, t.z0], ...teeth, ...top.insulator.slice(1, 3), ...top.contact.slice(1)];
}

/**
 * The socket cut through its axis on one side (angle `theta`): its profile with the thread's
 * teeth, [radius, height above the face] from the face's middle round to the gland's top.
 * @param {ReturnType<typeof socketOf>} f
 * @param {number} [theta]
 */
export function socketSection(f, theta = 0) {
  const t = socketThread(f);
  const profile = socketProfile(f);
  if (!t) return profile;
  const at = profile.findIndex(([r, z]) => r === t.root && z === t.z0);
  const steps = Math.max(2, Math.ceil(((t.z1 - t.z0) / t.pitch) * 8));
  const teeth = [];
  for (let k = 0; k <= steps; k++) {
    const z = t.z0 + ((t.z1 - t.z0) * k) / steps;
    teeth.push([threadRadius(t, theta, z), z]);
  }
  // In place of the root's straight run: the teeth.
  return [...profile.slice(0, at), ...teeth, ...profile.slice(at + 2)];
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
  // The bulb unscrewed from the socket (taken apart), this far below its face.
  const drop = /** @type {any} */ (fx).bulbDrop || 0;
  // The socket's section shows the thread: teeth on the right as cut at angle 0, on the left
  // at half a turn (half a pitch higher).
  const section = [...socketSection(fx, 0).map(([r, s]) => [r, z(s)]), ...socketSection(fx, Math.PI).map(([r, s]) => [-r, z(s)]).reverse()];
  return {
    socket: section,
    // The cap, screwed in (inside the holder, but for its bottom) or out with the bulb: its thread.
    cap: [...capSection(fx, 0).map(([r, s]) => [r, z(s - drop)]), ...capSection(fx, Math.PI).map(([r, s]) => [-r, z(s - drop)]).reverse()],
    bulb: both(bulbProfile(fx), (s) => -(drop + CAP_SHOWS + s)),
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
  /**
   * How big the bulb's light is, as a share of its glass (1: a frosted bulb glows over its whole
   * glass and the shade's lines and patterns blur on the wall; 0: a point, a clear filament bulb,
   * that throws them sharp).
   */
  lightSize: 1,
  /** The light round the lamp while the bulb is on (1: as with it off). */
  room: 0.06,
  /** Exposure, ±stops (a shade lit from inside shows its glow a stop up). */
  exposure: 1,
  /** Light bounces (the same for the preview and the render, so the preview shows the light as it'll be). */
  bounces: 12,
  /** The quick look and the render: the picture's longer side in pixels, and samples per pixel. */
  preview: { size: 640, samples: 32 },
  final: { size: 1920, samples: 512 },
  /**
   * Denoise each finished (or stopped) preview or render by itself (off: only when Denoise is
   * pressed), and how much (1: all denoised; less blends the picture as rendered back in).
   */
  denoise: false,
  denoiseStrength: 1,
  /**
   * The camera: where it looks from (`view`, for Fit), its lens (mm, as on a full-frame camera),
   * and where it was left (the scene's metres; null: fitted).
   */
  camera: { view: 'front', lens: 50, position: /** @type {number[] | null} */ (null), target: /** @type {number[] | null} */ (null) },
  /**
   * Surfaces round the lamp, lit by it and in the picture: a wall behind it (mm from the lamp's
   * axis; 0 cuts through the axis, less than 0 in front of it), a floor or table under it (mm below its lowest point), a ceiling above it (mm above
   * the socket's top; the cable ends there). One colour for all.
   */
  wall: { on: false, distance: 300 },
  floor: { on: false, distance: 500 },
  ceiling: { on: false, distance: 600 },
  surface: '#e9e5df',
};

/** Camera views (directions it looks from, y up, the front towards +z). */
export const CAMERA_VIEWS = {
  front: { label: 'Front', dir: [0, 0.06, 1] },
  'three-quarter': { label: '¾', dir: [0.62, 0.22, 0.75] },
  side: { label: 'Side', dir: [1, 0.06, 0] },
  below: { label: 'Below', dir: [0, -0.6, 0.8] },
  above: { label: 'Above', dir: [0, 0.65, 0.76] },
};

/**
 * Render settings from anything (a file, storage), every value checked and within its range;
 * what's missing or wrong takes its default.
 * @param {any} raw
 * @returns {typeof RENDER_DEFAULTS}
 */
export function cleanRender(raw) {
  const r = raw && typeof raw === 'object' ? raw : {};
  const D = RENDER_DEFAULTS;
  const num = (v, lo, hi, d) => {
    const x = Number(v);
    return v !== null && v !== undefined && v !== '' && Number.isFinite(x) ? Math.min(hi, Math.max(lo, x)) : d;
  };
  const obj = (v) => (v && typeof v === 'object' ? v : {});
  const quality = (q, d) => ({ size: Math.round(num(obj(q).size, 160, 4096, d.size)), samples: Math.round(num(obj(q).samples, 1, 10000, d.samples)) });
  const surface = (s, d, lo = 0) => ({ on: typeof obj(s).on === 'boolean' ? obj(s).on : d.on, distance: num(obj(s).distance, lo, 5000, d.distance) });
  const point = (p) => (Array.isArray(p) && p.length === 3 && p.every((x) => Number.isFinite(Number(x)) && Math.abs(Number(x)) < 1000) ? p.map(Number) : null);
  const cam = obj(r.camera);
  const position = point(cam.position);
  const target = point(cam.target);
  return {
    light: typeof r.light === 'boolean' ? r.light : D.light,
    lumens: num(r.lumens, 50, 5000, D.lumens),
    kelvin: num(r.kelvin, 1800, 6500, D.kelvin),
    lightSize: num(r.lightSize, 0, 1, D.lightSize),
    room: num(r.room, 0, 1, D.room),
    exposure: num(r.exposure, -6, 6, D.exposure),
    bounces: Math.round(num(r.bounces, 1, 32, D.bounces)),
    preview: quality(r.preview, D.preview),
    final: quality(r.final, D.final),
    denoise: typeof r.denoise === 'boolean' ? r.denoise : D.denoise,
    denoiseStrength: num(r.denoiseStrength, 0, 1, D.denoiseStrength),
    camera: {
      view: Object.hasOwn(CAMERA_VIEWS, String(cam.view)) ? String(cam.view) : D.camera.view,
      lens: num(cam.lens, 10, 300, D.camera.lens),
      position: position && target ? position : null,
      target: position && target ? target : null,
    },
    // The wall can cut through the lamp: 0 through its axis, less in front of it.
    wall: surface(r.wall, D.wall, -1000),
    floor: surface(r.floor, D.floor),
    ceiling: surface(r.ceiling, D.ceiling),
    surface: /^#[0-9a-f]{6}$/i.test(String(r.surface)) ? String(r.surface) : D.surface,
  };
}
