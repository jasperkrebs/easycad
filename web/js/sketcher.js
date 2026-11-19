// The Thread Lampshade Sketcher's settings files (a friend of the owner's design tool, 16 Nov
// 2026), read and written: its pendant, wings threading onto the socket, is the Thread pendant
// template, setting for setting. Plain functions, so the chat, the file picker and the tests
// share them. See docs/thread-pendant.md.

/** The template the sketcher's files open as. */
export const SKETCHER_TEMPLATE = 'thread-pendant';

/** The sketcher's socket presets: barrel Ø, thread pitch, body height. */
const SOCKETS = { E27: [40, 2.5, 55], E14: [28, 2, 45] };

/** The sketcher's own defaults, for a file that leaves something out and for writing one. */
export const SKETCHER_DEFAULTS = {
  profD: 30, profClr: 0.5, capD: 50, endLen: 30, wingRot: 0, hangLen: 700, linOrient: 'hang', footD: 260, footH: 15, footMat: 'steel',
  lamp: 'pendant', archN: 1, archH: 15, archW: 90, archSoft: 1, archAng: 270, minLayer: 1.5,
  path: [{ u: 0, w: 0 }, { u: 0.5, w: 0.5 }, { u: 1, w: 1 }], pathLen: 80,
  globeOpen: 45, globeTop: 45, globeBot: 45, shapeKind: '',
  H: 200, scale: 1, diam: 110, hD: 110, hZ: 0, profile: [0, 0.2, 0.4, 0.6, 0.8, 1].map((u) => ({ u, r: 52.5 })),
  wing: [{ u: 0, l: 0 }, { u: 0.25, l: 0.45 }, { u: 0.5, l: 1 }, { u: 1, l: 1 }],
  N: 2, wingLen: 147.5, maxOver: 45, tipGap: 3, neckGap: 0, wallGap: 0, pinGap: 0,
  mount: 'socket', socket: 'E27', grip: 25, hide: 35, sockH: 55, topMode: 'crease', crease: 5, mode: 'follow', threadD: 80, zMode: 'angle',
  cView: 'place', floor: true, Hc: 25, spokes: 1, eShade: 1, sockD: 40, eSock: 1, sockP: 2.5,
  bulb: 'A60', section: false, res: 'draft', LH: 2.5,
};

/**
 * Whether a parsed JSON file is the sketcher's settings.
 * @param {any} d
 */
export function isSketcherSettings(d) {
  return !!d && typeof d === 'object' && Array.isArray(d.profile) && Array.isArray(d.wing) && 'N' in d && ('sockD' in d || 'socket' in d);
}

/** A number kept to 6 decimals (the sketcher's handles are full doubles). */
const r6 = (x) => Math.round(Number(x) * 1e6) / 1e6;
const num = (x, fallback) => (Number.isFinite(Number(x)) ? Number(x) : fallback);

/**
 * The Thread pendant's setting values for a sketcher settings file, and what it couldn't take.
 * @param {any} file
 * @returns {{ values: Record<string, any>, notes: string[] }}
 */
export function sketcherToValues(file) {
  const d = { ...SKETCHER_DEFAULTS, ...file };
  const notes = [];
  if (d.lamp && d.lamp !== 'pendant') notes.push(`It's a ${d.lamp === 'floor' ? 'floor lamp' : 'linear pendant'} in the sketcher: only the pendant is made here, so it opens as one.`);
  if (d.mount && d.mount !== 'socket') notes.push('Its wings use a separate screw part in the sketcher: here they thread onto the socket.');
  const scale = num(d.scale, 1);
  const profile = [...d.profile]
    .sort((a, b) => a.u - b.u)
    .map((p) => (p.sharp ? [r6(p.u), r6(num(p.r, 50) * scale), 1] : [r6(p.u), r6(num(p.r, 50) * scale)]));
  const wing = [...d.wing].sort((a, b) => a.u - b.u).map((p) => [r6(p.u), r6(num(p.l, 0))]);
  const path = [...(Array.isArray(d.path) ? d.path : SKETCHER_DEFAULTS.path)].sort((a, b) => a.u - b.u).map((p) => [r6(p.u), r6(num(p.w, 0))]);
  const sock = [num(d.sockD, 40), num(d.sockP, num(d.LH, 2.5)), num(d.sockH, 55)];
  const preset = Object.entries(SOCKETS).find(([name, v]) => name === d.socket && v.every((x, i) => Math.abs(x - sock[i]) < 1e-9));
  const values = {
    socket: preset ? preset[0] : 'custom',
    socket_d: sock[0],
    socket_pitch: sock[1],
    socket_h: sock[2],
    engagement: num(d.eSock, 1),
    grip: num(d.grip, 25),
    hide: num(d.hide, 35),
    top: d.topMode === 'full' ? 'full' : 'crease',
    crease: num(d.crease, 5),
    // The sketcher's "angle": every layer rises a layer a turn, every ring a true helix, no seam.
    z_by: d.zMode === 'length' ? 'length' : 'angle',
    height: num(d.H, 200),
    profile,
    entries: Math.round(num(d.N, 2)),
    wing_zone: num(d.wingLen, 147.5),
    wing_curve: wing,
    max_overhang: num(d.maxOver, 45),
    tip_gap: num(d.tipGap, 3),
    neck_gap: num(d.neckGap, 0),
    wall_gap: num(d.wallGap, 0),
    pin_gap: num(d.pinGap, 0),
    rotation: num(d.wingRot, 0),
    path,
    path_len: num(d.pathLen, 80),
    resolution: d.res === 'fine' ? 'fine' : 'draft',
  };
  return { values, notes };
}

/**
 * A sketcher settings file for the Thread pendant's values (the sketcher's own keys; what the
 * pendant doesn't have, its defaults), to open the design in the sketcher again.
 * @param {Record<string, any>} v
 */
export function valuesToSketcher(v) {
  const sock = v.socket in SOCKETS ? SOCKETS[v.socket] : [v.socket_d, v.socket_pitch, v.socket_h];
  const profile = (v.profile || []).map((p) => (p[2] ? { u: p[0], r: p[1], sharp: true } : { u: p[0], r: p[1] }));
  const maxR = Math.max(...profile.map((p) => p.r), 0);
  return {
    ...SKETCHER_DEFAULTS,
    lamp: 'pendant',
    mount: 'socket',
    socket: v.socket in SOCKETS ? v.socket : 'E27',
    sockD: sock[0],
    sockP: sock[1],
    LH: sock[1],
    sockH: sock[2],
    eSock: v.engagement,
    grip: v.grip,
    hide: v.hide,
    topMode: v.top === 'full' ? 'full' : 'crease',
    crease: v.crease,
    zMode: v.z_by === 'length' ? 'length' : 'angle',
    H: v.height,
    scale: 1,
    profile,
    diam: Math.round((maxR + 2.5) * 2),
    N: v.entries,
    wingLen: v.wing_zone,
    wing: (v.wing_curve || []).map((p) => ({ u: p[0], l: p[1] })),
    maxOver: v.max_overhang,
    tipGap: v.tip_gap,
    neckGap: v.neck_gap,
    wallGap: v.wall_gap,
    pinGap: v.pin_gap,
    wingRot: v.rotation,
    path: (v.path || []).map((p) => ({ u: p[0], w: p[1] })),
    pathLen: v.path_len,
    res: v.resolution === 'fine' ? 'fine' : 'draft',
  };
}
