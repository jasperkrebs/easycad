// Where a printed wall's beads touch: for the render, which treats the faces where one turn sits
// on the next as inside the plastic (light crosses them as if they weren't there), so the wall
// is one solid whose outside is the beads' round faces. Pure (no three.js), so the node tests
// can check it.

/**
 * For each point of a toolpath, whether a bead lies right on top of its bead (one layer height
 * up, its path within half a bead across) and right under it. "Up" is the point's up direction
 * (`ups`, sliced along the surface: the beads stand along the wall) or straight up. Travel moves
 * (width 0) neither cover nor are covered.
 *
 * Also where the bead above and the bead below sit across: `aboveAt`/`belowAt` hold, per point,
 * the offset to that bead's path square to up (x, y, z) and half the length of its flat face
 * (the flat underside of the bead above, the flat top of the bead below). On a sloping wall
 * sliced by height each turn sits partly beside the last, and only that part of a flat face is
 * inside the wall.
 * @param {Float32Array | number[]} points x, y, z per point
 * @param {Float32Array | number[]} widths line width per point (of the move into it)
 * @param {Float32Array | number[]} heights layer height per point
 * @param {Float32Array | number[] | null} [ups] up direction per point (x, y, z), or straight up
 * @returns {{ above: Uint8Array, below: Uint8Array, aboveAt: Float32Array, belowAt: Float32Array }}
 */
export function touching(points, widths, heights, ups = null) {
  const n = Math.floor(points.length / 3);
  const above = new Uint8Array(n);
  const below = new Uint8Array(n);
  const aboveAt = new Float32Array(4 * n);
  const belowAt = new Float32Array(4 * n);
  const aboveBest = new Float32Array(n).fill(Infinity);
  const belowBest = new Float32Array(n).fill(Infinity);
  // The printing moves (j-1 → j), hashed by the cells their footprint crosses.
  let cell = 1;
  for (let j = 1; j < n; j++) {
    if (!(widths[j] > 0)) continue;
    const len = Math.hypot(points[3 * j] - points[3 * j - 3], points[3 * j + 1] - points[3 * j - 2]);
    cell = Math.max(cell, widths[j], Math.min(len, 50));
  }
  /** @type {Map<string, number[]>} */
  const grid = new Map();
  const key = (cx, cy) => `${cx},${cy}`;
  for (let j = 1; j < n; j++) {
    if (!(widths[j] > 0)) continue;
    const [x0, y0, x1, y1] = [points[3 * j - 3], points[3 * j - 2], points[3 * j], points[3 * j + 1]];
    const [ax, bx] = [Math.floor(Math.min(x0, x1) / cell), Math.floor(Math.max(x0, x1) / cell)];
    const [ay, by] = [Math.floor(Math.min(y0, y1) / cell), Math.floor(Math.max(y0, y1) / cell)];
    if ((bx - ax + 1) * (by - ay + 1) > 64) continue; // a long jump, not a bead to sit on
    for (let cx = ax; cx <= bx; cx++) {
      for (let cy = ay; cy <= by; cy++) {
        const k = key(cx, cy);
        const list = grid.get(k);
        if (list) list.push(j);
        else grid.set(k, [j]);
      }
    }
  }
  for (let i = 0; i < n; i++) {
    // A point's bead: the move into it, or (the run's first point) the move out of it.
    const w = widths[i] > 0 ? widths[i] : i + 1 < n ? widths[i + 1] : 0;
    const h = widths[i] > 0 ? heights[i] : i + 1 < n ? heights[i + 1] : 0;
    if (!(w > 0 && h > 0)) continue;
    const [x, y, z] = [points[3 * i], points[3 * i + 1], points[3 * i + 2]];
    let [vx, vy, vz] = ups ? [ups[3 * i], ups[3 * i + 1], ups[3 * i + 2]] : [0, 0, 1];
    const vl = Math.hypot(vx, vy, vz);
    [vx, vy, vz] = vl > 1e-9 ? [vx / vl, vy / vl, vz / vl] : [0, 0, 1];
    const cx = Math.floor(x / cell);
    const cy = Math.floor(y / cell);
    for (let dx = -1; dx <= 1; dx++) {
      for (let dy = -1; dy <= 1; dy++) {
        for (const j of grid.get(key(cx + dx, cy + dy)) || []) {
          if (j === i || j - 1 === i) continue;
          // The nearest point of move j across (square to up), and how far up it is there.
          const [x0, y0, z0, x1, y1, z1] = [points[3 * j - 3], points[3 * j - 2], points[3 * j - 1], points[3 * j], points[3 * j + 1], points[3 * j + 2]];
          const flatOf = (v) => {
            const d = v[0] * vx + v[1] * vy + v[2] * vz;
            return [v[0] - d * vx, v[1] - d * vy, v[2] - d * vz];
          };
          const u = flatOf([x1 - x0, y1 - y0, z1 - z0]);
          const r = flatOf([x - x0, y - y0, z - z0]);
          const len2 = u[0] * u[0] + u[1] * u[1] + u[2] * u[2];
          const t = len2 > 1e-12 ? Math.min(1, Math.max(0, (r[0] * u[0] + r[1] * u[1] + r[2] * u[2]) / len2)) : 0;
          const off = [x0 + t * (x1 - x0) - x, y0 + t * (y1 - y0) - y, z0 + t * (z1 - z0) - z];
          const up = off[0] * vx + off[1] * vy + off[2] * vz;
          const [ox, oy, oz] = flatOf(off);
          const across = Math.hypot(ox, oy, oz);
          if (across > 0.5 * Math.max(w, widths[j])) continue;
          const flat = Math.max(0, widths[j] / 2 - Math.min(widths[j], heights[j]) / 2);
          if (Math.abs(up - h) < 0.5 * h) {
            above[i] = 1;
            if (across < aboveBest[i]) {
              aboveBest[i] = across;
              aboveAt.set([ox, oy, oz, flat], 4 * i);
            }
          }
          if (Math.abs(up + heights[j]) < 0.5 * heights[j]) {
            below[i] = 1;
            if (across < belowBest[i]) {
              belowBest[i] = across;
              belowAt.set([ox, oy, oz, flat], 4 * i);
            }
          }
        }
      }
    }
  }
  return { above, below, aboveAt, belowAt };
}
