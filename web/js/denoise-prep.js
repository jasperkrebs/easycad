// The denoiser's help on either side of the network (Open Image Denoise, render.js), pure so the
// node tests can check it. Pictures are RGBA floats, linear light, row by row.
//
// Before: fireflies tamed. A path that finds the bulb through a lens of plastic now and then
// brings back a pixel tens of times too bright. The display clips it to a white speck, but the
// network spreads its light round it: blotches, and the whole picture brighter than it looks
// as rendered. A pixel far brighter than all but one of its neighbours is brought down to a few
// times that neighbour (an edge, the bulb's outline, has bright neighbours, and stays).
//
// After: the detail kept. The network takes what its guides (albedo, normals) don't show for
// noise: the light the layer lines and a woven pattern bend through a see-through shade, the
// glints along the beads. What it took away, smoothed over a few pixels (the noise mostly
// cancels, a line several pixels long stays), can be added back: `detail`.

/** Luminance of RGBA pixel i (its index ×4). */
const lum = (c, i) => 0.2126 * c[i] + 0.7152 * c[i + 1] + 0.0722 * c[i + 2];

/**
 * Brings down fireflies in place: a pixel brighter than `k` × its second-brightest neighbour.
 * @param {Float32Array} c RGBA
 * @param {number} w
 * @param {number} h
 * @param {number} [k]
 * @returns {number} how many pixels were brought down
 */
export function tameFireflies(c, w, h, k = 4) {
  const l = new Float32Array(w * h);
  for (let p = 0; p < w * h; p++) l[p] = lum(c, 4 * p);
  let tamed = 0;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const p = y * w + x;
      const v = l[p];
      if (!(v > 0)) continue;
      // The two brightest neighbours.
      let [a, b] = [0, 0];
      for (let dy = -1; dy <= 1; dy++) {
        const yy = y + dy;
        if (yy < 0 || yy >= h) continue;
        for (let dx = -1; dx <= 1; dx++) {
          const xx = x + dx;
          if ((dx === 0 && dy === 0) || xx < 0 || xx >= w) continue;
          const n = l[yy * w + xx];
          if (n > a) [a, b] = [n, a];
          else if (n > b) b = n;
        }
      }
      const limit = k * b;
      if (v > limit) {
        const s = limit / v;
        c[4 * p] *= s;
        c[4 * p + 1] *= s;
        c[4 * p + 2] *= s;
        tamed++;
      }
    }
  }
  return tamed;
}

/**
 * What the network took away (as rendered − denoised, colour only), smoothed over about two
 * pixels each way (a 5-tap binomial blur, across then down).
 * @param {Float32Array} raw RGBA as rendered (fireflies tamed)
 * @param {Float32Array} den RGBA denoised
 * @param {number} w
 * @param {number} h
 */
export function takenDetail(raw, den, w, h) {
  const taps = [1 / 16, 4 / 16, 6 / 16, 4 / 16, 1 / 16];
  const diff = new Float32Array(raw.length);
  for (let i = 0; i < raw.length; i++) diff[i] = i % 4 === 3 ? 0 : raw[i] - den[i];
  const across = new Float32Array(raw.length);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      for (let ch = 0; ch < 3; ch++) {
        let s = 0;
        for (let t = -2; t <= 2; t++) s += taps[t + 2] * diff[4 * (y * w + Math.min(w - 1, Math.max(0, x + t))) + ch];
        across[4 * (y * w + x) + ch] = s;
      }
    }
  }
  const out = new Float32Array(raw.length);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      for (let ch = 0; ch < 3; ch++) {
        let s = 0;
        for (let t = -2; t <= 2; t++) s += taps[t + 2] * across[4 * (Math.min(h - 1, Math.max(0, y + t)) * w + x) + ch];
        out[4 * (y * w + x) + ch] = s;
      }
    }
  }
  return out;
}

/**
 * The picture shown: as rendered blended toward denoised by `strength` (0..1), with `detail`
 * (0..1) of what the network took away added back to the denoised one. Alpha: blended only.
 * @param {Float32Array} out
 * @param {Float32Array} raw
 * @param {Float32Array} den
 * @param {Float32Array | null} detailTaken from `takenDetail`
 * @param {number} strength
 * @param {number} detail
 */
export function blendDenoised(out, raw, den, detailTaken, strength, detail) {
  const t = Math.min(1, Math.max(0, strength));
  const d = detailTaken ? Math.min(1, Math.max(0, detail)) : 0;
  for (let i = 0; i < out.length; i++) {
    const clean = i % 4 === 3 || !detailTaken ? den[i] : den[i] + d * detailTaken[i];
    out[i] = Math.max(0, raw[i] + (clean - raw[i]) * t);
  }
}
