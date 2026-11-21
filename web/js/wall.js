// Where a printed wall's beads touch: for the render, which treats the faces where one turn sits
// on the next as inside the plastic (light crosses them as if they weren't there), so the wall
// is one solid whose outside is the beads' round faces. Pure (no three.js), so the node tests
// can check it.

/**
 * For each point of a toolpath, whether a bead lies right on top of its bead (one layer height
 * up, its path within half a bead across) and right under it. Travel moves (width 0) neither
 * cover nor are covered.
 * @param {Float32Array | number[]} points x, y, z per point
 * @param {Float32Array | number[]} widths line width per point (of the move into it)
 * @param {Float32Array | number[]} heights layer height per point
 * @returns {{ above: Uint8Array, below: Uint8Array }}
 */
export function touching(points, widths, heights) {
  const n = Math.floor(points.length / 3);
  const above = new Uint8Array(n);
  const below = new Uint8Array(n);
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
    const cx = Math.floor(x / cell);
    const cy = Math.floor(y / cell);
    for (let dx = -1; dx <= 1 && !(above[i] && below[i]); dx++) {
      for (let dy = -1; dy <= 1; dy++) {
        for (const j of grid.get(key(cx + dx, cy + dy)) || []) {
          if (j === i || j - 1 === i) continue;
          // The nearest point of move j across (in plan), and its height there.
          const [x0, y0, z0, x1, y1, z1] = [points[3 * j - 3], points[3 * j - 2], points[3 * j - 1], points[3 * j], points[3 * j + 1], points[3 * j + 2]];
          const [ux, uy] = [x1 - x0, y1 - y0];
          const len2 = ux * ux + uy * uy;
          const t = len2 > 1e-12 ? Math.min(1, Math.max(0, ((x - x0) * ux + (y - y0) * uy) / len2)) : 0;
          const across = Math.hypot(x - (x0 + t * ux), y - (y0 + t * uy));
          if (across > 0.5 * Math.max(w, widths[j])) continue;
          const dz = z0 + t * (z1 - z0) - z;
          if (Math.abs(dz - h) < 0.5 * h) above[i] = 1;
          if (Math.abs(dz + heights[j]) < 0.5 * heights[j]) below[i] = 1;
        }
      }
    }
  }
  return { above, below };
}
