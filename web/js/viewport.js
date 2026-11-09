// 3D view: three.js drawing on the GPU (WebGL 2 today; three's WebGPURenderer is the drop-in
// upgrade when we need GPU compute or multi-million-segment toolpaths). Z is up, like Rhino and
// slicers. It renders only when something changes, which saves battery on phones.
//
// Parallel (orthographic, true to proportion like a technical drawing) by default, or
// perspective; and set views (3D, front, side, top), as in slicers. Toolpaths are lines coloured
// by height, or solid beads at their real line width and layer height, as slicers draw them.

import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { Line2 } from 'three/addons/lines/Line2.js';
import { LineGeometry } from 'three/addons/lines/LineGeometry.js';
import { LineMaterial } from 'three/addons/lines/LineMaterial.js';

/** @param {string} name CSS custom property, e.g. '--accent' */
function token(name) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || '#888888';
}

/**
 * @param {HTMLCanvasElement} canvas
 */
export function createViewport(canvas) {
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));

  const scene = new THREE.Scene();
  const persp = new THREE.PerspectiveCamera(35, 1, 1, 20000);
  const ortho = new THREE.OrthographicCamera(-1, 1, 1, -1, 1, 20000);
  for (const c of [persp, ortho]) {
    c.up.set(0, 0, 1);
    c.position.set(300, -380, 260);
  }
  /** The camera on screen: `ortho` (parallel) or `persp`. */
  /** @type {THREE.PerspectiveCamera | THREE.OrthographicCamera} */
  let camera = ortho;
  /** Half the height the parallel view shows at zoom 1 (mm), and the view's width ÷ height. */
  let orthoHalf = 100;
  let aspect = 1;
  const tanHalf = () => Math.tan(THREE.MathUtils.degToRad(persp.fov / 2));

  const skyLight = new THREE.HemisphereLight(0xffffff, 0x3a3f4a, 1.5);
  scene.add(skyLight);
  const keyLight = new THREE.DirectionalLight(0xffffff, 1.6);
  scene.add(keyLight, keyLight.target);

  const controls = new OrbitControls(camera, canvas);
  controls.addEventListener('change', requestRender);

  // Shown surfaces: a mesh per printed part (and one for what belongs to none), so a part can be
  // hidden. Each is drawn twice: its outside, and its other side, so an open surface (a
  // lampshade, a cone seen from above) shows its inside, in a shade blended toward the
  // background so inside and outside read apart (back faces only: three.js lights them with
  // the normals turned around).
  const meshMaterial = new THREE.MeshStandardMaterial({ roughness: 0.55, metalness: 0.0 });
  const insideMaterial = new THREE.MeshStandardMaterial({ roughness: 0.7, metalness: 0.0, side: THREE.BackSide });
  const shapes = new THREE.Group();
  scene.add(shapes);

  // Toolpaths: fat lines coloured by height, or (built only while they're shown) solid beads.
  // Curves: thin lines.
  const lineMaterial = new LineMaterial({ linewidth: 1.1, vertexColors: true, worldUnits: false });
  // To show the print from a chosen move on, segments before `clipFrom` (each toolpath's own,
  // set just before it's drawn) are moved off screen.
  lineMaterial.uniforms.clipFrom = { value: 0 };
  lineMaterial.vertexShader = lineMaterial.vertexShader.replace(
    'void main() {',
    'uniform float clipFrom;\nvoid main() {\n\tif ( float( gl_InstanceID ) < clipFrom ) { gl_Position = vec4( 0.0, 0.0, 2.0, 1.0 ); return; }',
  );
  const paths = new THREE.Group();
  // Slightly see-through, so the beads behind and the inside of the part show through them.
  const beadMaterial = new THREE.MeshStandardMaterial({
    vertexColors: true,
    roughness: 0.6,
    metalness: 0.0,
    side: THREE.DoubleSide,
    transparent: true,
    opacity: 0.4,
    depthWrite: false,
  });
  const beads = new THREE.Group();
  const curveMaterial = new THREE.LineBasicMaterial();
  const curves = new THREE.Group();
  scene.add(paths, beads, curves);

  /** @type {THREE.GridHelper | null} */
  let grid = null;

  let mode = 'both';
  /** Toolpaths drawn as solid beads (real width and height) instead of lines. */
  let showBeads = false;
  /** Bed width and depth, and where the recipe's origin sits on it (all in mm). */
  let bed = { size: [256, 256], center: [125, 125] };
  let fitted = false;
  /** @type {{ min: number[], max: number[] } | null} */
  let lastBounds = null;
  let frame = 0;
  /** Printed parts hidden from view (indices into the result's prints). */
  let hiddenParts = new Set();

  function requestRender() {
    if (!frame) frame = requestAnimationFrame(render);
  }

  function render() {
    frame = 0;
    // A light that follows the camera (up and to the left of it) keeps every side readable.
    keyLight.position.copy(camera.position).add(new THREE.Vector3(-0.3, 0.2, 0.6).multiplyScalar(camera.position.length()));
    keyLight.target.position.copy(controls.target);
    renderer.render(scene, camera);
  }

  /** The parallel view's box, from its half height and the view's shape. */
  function frameOrtho() {
    ortho.left = -orthoHalf * aspect;
    ortho.right = orthoHalf * aspect;
    ortho.top = orthoHalf;
    ortho.bottom = -orthoHalf;
    ortho.updateProjectionMatrix();
  }

  function resize() {
    const w = Math.max(1, canvas.clientWidth);
    const h = Math.max(1, canvas.clientHeight);
    renderer.setSize(w, h, false);
    aspect = w / h;
    persp.aspect = aspect;
    persp.updateProjectionMatrix();
    frameOrtho();
    lineMaterial.resolution.set(w, h);
    headLineMaterial.resolution.set(w, h);
    beadEdgeMaterial.resolution.set(w, h);
    requestRender();
  }
  new ResizeObserver(resize).observe(canvas);

  function rebuildGrid() {
    if (grid) {
      scene.remove(grid);
      grid.geometry.dispose();
      /** @type {THREE.Material} */ (grid.material).dispose();
    }
    const c = new THREE.Color(token('--view-grid'));
    // A unit grid stretched to the bed. GridHelper lies in XZ; our bed is the XY plane. The
    // bed's corner is at -center in recipe coordinates, where the printer puts the origin.
    const [w, d] = bed.size;
    grid = new THREE.GridHelper(1, 8, c, c);
    grid.scale.set(w, 1, d);
    grid.rotation.x = Math.PI / 2;
    grid.position.set(w / 2 - bed.center[0], d / 2 - bed.center[1], 0);
    scene.add(grid);
  }

  /**
   * What the toolpath can be coloured by: each move's height, speed (mm/s, as the G-code
   * writes it), volumetric flow (mm³/s), line width or layer height (mm).
   */
  const COLOR_MODES = {
    height: { label: 'Height', unit: 'mm', digits: 1 },
    speed: { label: 'Speed', unit: 'mm/s', digits: 1 },
    flow: { label: 'Flow', unit: 'mm³/s', digits: 1 },
    width: { label: 'Line width', unit: 'mm', digits: 2 },
    layer: { label: 'Layer height', unit: 'mm', digits: 2 },
    simWidth: { label: 'Simulated width', unit: 'mm', digits: 2 },
    squish: { label: 'Squish (simulated ÷ planned width)', unit: '×', digits: 2 },
    custom: { label: 'Custom', unit: '', digits: 0 },
  };
  /**
   * The custom colours, for simple renders of several parts: each printed part's colour and how
   * solid it is (by its index among the prints), and the light: how much white the highlights
   * get and how deep (black) the shadows go, 0..1 (½ each: the view's usual light).
   */
  let custom = { parts: /** @type {{ color: string, opacity: number }[]} */ ([]), light: 0.5, shadow: 0.5 };
  /** Colours for parts with none chosen yet (the plan's). */
  const PART_COLORS = ['#4f9dff', '#2bd9a0', '#ffb454', '#b58cff', '#3fd0d4', '#ff7ab6'];
  const partLook = (part) => custom.parts[part] || { color: PART_COLORS[Math.max(0, part) % PART_COLORS.length], opacity: 1 };
  /** Low to high, as slicers show speeds and widths: blue, cyan, green, yellow, red. */
  const SCALE = ['#3b4cc0', '#2fa4d9', '#5cc46a', '#f2c84b', '#d7372b'].map((c) => new THREE.Color(c));
  /** @type {keyof typeof COLOR_MODES} */
  let colorMode = 'height';
  /** The colour range of what's shown: [lowest, highest] of the colour mode's value. */
  let range = [0, 1];
  /** The range of every printing move, before any set by hand. */
  let autoRange = [0, 1];
  /** Ranges set by hand, per colour mode: [low, high], either one null for automatic. */
  const ranges = /** @type {Record<string, [number | null, number | null]>} */ ({});

  /** The value move `i` of a toolpath is coloured by (null: a travel, drawn grey). */
  function valueAt(data, i) {
    const travel = data.widths && !(data.widths[i] > 0);
    if (colorMode === 'height') return data.points[3 * i + 2];
    if (travel) return null;
    if (colorMode === 'squish') return data.simWidths && data.widths[i] > 0 ? data.simWidths[i] / data.widths[i] : null;
    const list = { speed: data.speeds, flow: data.flows, width: data.widths, layer: data.heights, simWidth: data.simWidths }[colorMode];
    return list ? list[i] : null;
  }

  /** Beads drawn as simulated (how the plastic would actually lie) rather than as planned. */
  let simulated = false;
  /** The bead sizes drawn for a toolpath: simulated when that's on and there, else planned. */
  const beadSizes = (data) =>
    simulated && data.simWidths ? { widths: data.simWidths, heights: data.simHeights } : { widths: data.widths, heights: data.heights };

  /**
   * Which way the nozzle and the beads stand: 'upright' (a Cartesian printer's), 'tilt' (along
   * the wall, at most 45° from upright: a robot arm with pellets, which need gravity) or 'wall'
   * (along the wall however far it leans). Preview only: the G-code is the same.
   */
  let orientation = 'upright';
  const MAX_TILT = Math.PI / 4;
  /**
   * Each point's up direction for the beads and nozzle (x, y, z per point), or null when upright.
   * From the engine's way up the wall (`paths.ups`), limited to 45° in 'tilt'.
   * @returns {Float32Array | null}
   */
  function upsFor(data) {
    if (orientation === 'upright' || !data.ups) return null;
    if (data.upsFor === orientation) return data.tilted;
    const out = new Float32Array(data.ups);
    if (orientation === 'tilt') {
      const [c, s] = [Math.cos(MAX_TILT), Math.sin(MAX_TILT)];
      for (let i = 0; i < out.length; i += 3) {
        if (out[i + 2] >= c) continue;
        const h = Math.hypot(out[i], out[i + 1]);
        if (h < 1e-9) continue;
        out[i] = (out[i] / h) * s;
        out[i + 1] = (out[i + 1] / h) * s;
        out[i + 2] = c;
      }
    }
    data.upsFor = orientation;
    data.tilted = out;
    return out;
  }

  /** The range of the colour mode's value over every printing move shown. */
  function colorRange() {
    let lo = Infinity;
    let hi = -Infinity;
    for (const line of paths.children) {
      const data = line.userData;
      const n = data.points.length / 3;
      for (let i = 0; i < n; i++) {
        // Printing moves only: travels (hops high over the print) don't stretch the range.
        if (data.widths && !(data.widths[i] > 0)) continue;
        const v = valueAt(data, i);
        if (v == null || !Number.isFinite(v)) continue;
        if (v < lo) lo = v;
        if (v > hi) hi = v;
      }
    }
    return Number.isFinite(lo) ? [lo, hi] : [0, 1];
  }

  /** A colour for value `v` in the range, on the scale (the same for every colour mode). */
  function colorFor(v, out) {
    const [lo, hi] = range;
    const t = hi > lo ? Math.min(1, Math.max(0, (v - lo) / (hi - lo))) : 0.5;
    const x = t * (SCALE.length - 1);
    const k = Math.min(SCALE.length - 2, Math.floor(x));
    return out.copy(SCALE[k]).lerp(SCALE[k + 1], x - k);
  }
  const travelColor = new THREE.Color();

  function pathColors(data) {
    const n = data.points.length / 3;
    const out = new Float32Array(n * 3);
    const c = new THREE.Color();
    const own = new THREE.Color(partLook(data.part ?? -1).color);
    for (let i = 0; i < n; i++) {
      const travel = data.widths && !(data.widths[i] > 0);
      const v = colorMode === 'custom' ? (travel ? null : 0) : valueAt(data, i);
      if (v == null) c.copy(travelColor);
      else if (colorMode === 'custom') c.copy(own);
      else colorFor(v, c);
      out[3 * i] = c.r;
      out[3 * i + 1] = c.g;
      out[3 * i + 2] = c.b;
    }
    return out;
  }

  function recolorPaths() {
    travelColor.set(token('--view-grid'));
    autoRange = colorRange();
    const set = ranges[colorMode] || [null, null];
    range = [set[0] ?? autoRange[0], set[1] ?? autoRange[1]];
    for (const line of paths.children) {
      const geometry = /** @type {LineGeometry} */ (/** @type {Line2} */ (line).geometry);
      geometry.setColors(pathColors(line.userData));
    }
    if (showBeads) buildBeads();
    applyProgress();
  }

  /** Steps per half circle of a bead's section (more: rounder, heavier). */
  const ROUND = 5;
  /** Points round a bead's section: each half circle's, both ends included. */
  const RING = 2 * (ROUND + 1);
  /** Turns sharper than this get a round joint, the section pivoting round the corner. */
  const SHARP = THREE.MathUtils.degToRad(25);
  /** Pivot steps: one per this much turn. */
  const PIVOT_STEP = THREE.MathUtils.degToRad(15);
  /** Rings in a round end cap. */
  const CAP = 4;

  /**
   * One toolpath as solid beads, like a round nozzle laying plastic along it: each move swept
   * with its own bead (the G-code's model: a flat top and bottom with a half circle of the
   * layer height on each side, the line width across, from the nozzle down one layer height).
   * Where the path bends gently the section follows the bend; at a sharp corner it pivots round
   * the corner point, so the bead rounds the corner instead of notching it; each run of moves
   * ends in a round cap. Travel moves leave a gap. Also returns, for each point, how many
   * indices draw the bead up to that point (to show the print part way).
   * @param {Float32Array} points
   * @param {Float32Array} widths
   * @param {Float32Array} heights
   * @param {Float32Array} colors
   * @param {Float32Array | null} [ups] each point's up direction (x, y, z), the beads' sections
   *   standing along it; straight up when not given
   */
  function beadGeometry(points, widths, heights, colors, ups = null) {
    const n = points.length / 3;
    /** @type {number[]} */
    const pos = [];
    /** @type {number[]} */
    const col = [];
    /** @type {number[]} */
    const index = [];
    const upTo = new Uint32Array(n);
    // The section's outline, as (across, up) for a bead `w` wide and `h` tall, centred.
    const section = (w, h) => {
      const r = Math.max(1e-6, Math.min(w, h) / 2);
      const c = Math.max(0, w / 2 - r);
      const out = [];
      for (let s = 0; s <= ROUND; s++) {
        const a = -Math.PI / 2 + (Math.PI * s) / ROUND;
        out.push([c + r * Math.cos(a), r * Math.sin(a)]);
      }
      for (let s = 0; s <= ROUND; s++) {
        const a = Math.PI / 2 + (Math.PI * s) / ROUND;
        out.push([-c + r * Math.cos(a), r * Math.sin(a)]);
      }
      return out;
    };
    let rings = 0;
    let joined = false;
    /**
     * A ring at (x, y, z) across (ax, ay), w wide and h tall, in point i's colour. `inner`
     * (at a sharp corner): the side of the section inside the turn stays at the corner's inner
     * mitre point instead of swinging round, so the joint doesn't fold over itself.
     * @param {{ bx: number, by: number, k: number, side: number } | null} [inner]
     */
    const ring = (x, y, z, ax, ay, w, h, i, inner = null) => {
      // The section's frame: across (A) and up (V). Upright, A is level and V straight up;
      // tilted, V is the point's up and A the level across direction turned square to it.
      let [Ax, Ay, Az, Vx, Vy, Vz] = [ax, ay, 0, 0, 0, 1];
      let [Bx, By, Bz] = inner ? [inner.bx, inner.by, 0] : [0, 0, 0];
      if (ups) {
        [Vx, Vy, Vz] = [ups[3 * i], ups[3 * i + 1], ups[3 * i + 2]];
        const square = (px, py) => {
          const d = px * Vx + py * Vy;
          const [qx, qy, qz] = [px - d * Vx, py - d * Vy, -d * Vz];
          const len = Math.hypot(qx, qy, qz) || 1;
          return [qx / len, qy / len, qz / len];
        };
        [Ax, Ay, Az] = square(ax, ay);
        if (inner) [Bx, By, Bz] = square(inner.bx, inner.by);
      }
      for (const [u, v] of section(w, h)) {
        const up = v - h / 2;
        if (inner && u * inner.side > 0) {
          const d = Math.abs(u) * inner.k;
          pos.push(x + Bx * d + Vx * up, y + By * d + Vy * up, z + Bz * d + Vz * up);
        } else {
          pos.push(x + Ax * u + Vx * up, y + Ay * u + Vy * up, z + Az * u + Vz * up);
        }
        col.push(colors[3 * i], colors[3 * i + 1], colors[3 * i + 2]);
      }
      if (joined) {
        const [p, q] = [RING * (rings - 1), RING * rings];
        for (let c = 0; c < RING; c++) {
          const d = (c + 1) % RING;
          index.push(p + c, q + c, q + d, p + c, q + d, p + d);
        }
      }
      rings++;
      joined = true;
    };
    // The direction of move i (into point i) in the layer; the last good one when it's vertical.
    const dirs = new Float32Array(2 * n);
    let [dx, dy] = [1, 0];
    for (let i = 1; i < n; i++) {
      const tx = points[3 * i] - points[3 * i - 3];
      const ty = points[3 * i + 1] - points[3 * i - 2];
      const len = Math.hypot(tx, ty);
      if (len > 1e-9) [dx, dy] = [tx / len, ty / len];
      dirs[2 * i] = dx;
      dirs[2 * i + 1] = dy;
    }
    const printing = (i) => i > 0 && i < n && widths[i] > 0;
    const at = (i) => [points[3 * i], points[3 * i + 1], points[3 * i + 2]];
    for (let i = 0; i < n; i++) {
      const into = printing(i);
      const out = printing(i + 1);
      if (!into && !out) {
        joined = false;
        upTo[i] = index.length;
        continue;
      }
      const [x, y, z] = at(i);
      if (!into) {
        // A run starts: a round cap behind the point, the first move's bead.
        const [ux, uy] = [dirs[2 * i + 2], dirs[2 * i + 3]];
        const [w, h] = [widths[i + 1], heights[i + 1]];
        joined = false;
        for (let c = 0; c <= CAP; c++) {
          const a = (Math.PI / 2) * (1 - c / CAP);
          const back = (w / 2) * Math.sin(a);
          ring(x - ux * back, y - uy * back, z, -uy, ux, w * Math.cos(a), h, i);
        }
      } else if (!out) {
        // A run ends: the last move's bead up to the point, then a round cap ahead of it.
        const [ux, uy] = [dirs[2 * i], dirs[2 * i + 1]];
        const [w, h] = [widths[i], heights[i]];
        for (let c = 0; c <= CAP; c++) {
          const a = (Math.PI / 2) * (c / CAP);
          const ahead = (w / 2) * Math.sin(a);
          ring(x + ux * ahead, y + uy * ahead, z, -uy, ux, w * Math.cos(a), h, i);
        }
      } else {
        // Between two moves: follow a gentle bend, pivot round a sharp one.
        const [ix, iy] = [dirs[2 * i], dirs[2 * i + 1]];
        const [ox, oy] = [dirs[2 * i + 2], dirs[2 * i + 3]];
        const turn = Math.atan2(ix * oy - iy * ox, ix * ox + iy * oy);
        const [w0, h0, w1, h1] = [widths[i], heights[i], widths[i + 1], heights[i + 1]];
        if (Math.abs(turn) < SHARP) {
          const bx = -(iy + oy);
          const by = ix + ox;
          const len = Math.hypot(bx, by) || 1;
          // Mitred, so the bead keeps its width through the bend.
          const mitre = 1 / Math.max(0.5, Math.cos(turn / 2));
          ring(x, y, z, bx / len, by / len, ((w0 + w1) / 2) * mitre, (h0 + h1) / 2, i);
        } else {
          const steps = Math.ceil(Math.abs(turn) / PIVOT_STEP);
          const a0 = Math.atan2(iy, ix);
          // Inside the turn: the bisector of the two moves' sides, out to where they meet.
          const side = Math.sign(turn);
          let bx = (-iy - oy) * side;
          let by = (ix + ox) * side;
          const blen = Math.hypot(bx, by);
          if (blen > 1e-9) {
            bx /= blen;
            by /= blen;
          } else {
            [bx, by] = [-ix, -iy]; // straight back on itself
          }
          const inner = { bx, by, k: 1 / Math.max(0.5, Math.cos(turn / 2)), side };
          for (let s = 0; s <= steps; s++) {
            const f = s / steps;
            const a = a0 + turn * f;
            ring(x, y, z, -Math.sin(a), Math.cos(a), w0 + (w1 - w0) * f, h0 + (h1 - h0) * f, i, inner);
          }
        }
      }
      upTo[i] = index.length;
      if (!out) joined = false;
    }
    const g = new THREE.BufferGeometry();
    g.setAttribute('position', new THREE.BufferAttribute(new Float32Array(pos), 3));
    g.setAttribute('color', new THREE.BufferAttribute(new Float32Array(col), 3));
    g.setIndex(new THREE.BufferAttribute(rings * RING > 65535 ? new Uint32Array(index) : new Uint16Array(index), 1));
    g.computeVertexNormals();
    return { geometry: g, upTo };
  }

  /** A part's own bead material in the custom colours: as solid as set for it. */
  function partBeadMaterial(part) {
    const o = partLook(part ?? -1).opacity;
    return new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.6, metalness: 0.0, side: THREE.DoubleSide, transparent: o < 1, opacity: o, depthWrite: o >= 1 });
  }

  /**
   * Shapes in the custom colours: each part's outside in its colour, its inside a shade darker,
   * both as solid as set for it; the light's highlights and shadows as set. Otherwise the
   * view's own look.
   */
  function applyCustom() {
    const on = colorMode === 'custom';
    for (const child of shapes.children) {
      const mesh = /** @type {THREE.Mesh} */ (child);
      const inside = mesh.userData.inside;
      if (mesh.userData.own) /** @type {THREE.Material} */ (mesh.material).dispose();
      mesh.userData.own = on;
      if (!on) {
        mesh.material = inside ? insideMaterial : meshMaterial;
        continue;
      }
      const look = partLook(mesh.userData.part);
      const color = new THREE.Color(look.color);
      if (inside) color.lerp(new THREE.Color(0x000000), 0.35);
      const o = look.opacity;
      mesh.material = new THREE.MeshStandardMaterial({ color, roughness: inside ? 0.7 : 0.55, metalness: 0.0, side: inside ? THREE.BackSide : THREE.FrontSide, transparent: o < 1, opacity: o, depthWrite: o >= 1 });
    }
    const light = on ? custom.light : 0.5;
    const shadow = on ? custom.shadow : 0.5;
    keyLight.intensity = 0.4 + 2.4 * light;
    skyLight.intensity = 2.6 - 2.2 * shadow;
    skyLight.groundColor.set(0x3a3f4a).lerp(new THREE.Color(0x000000), shadow);
    requestRender();
  }

  /** The beads for every toolpath shown as lines, in the same colours and parts. */
  function buildBeads() {
    clear(beads);
    for (const line of paths.children) {
      const data = line.userData;
      if (!data.widths || !data.heights) continue;
      const sizes = beadSizes(data);
      const { geometry, upTo } = beadGeometry(data.points, sizes.widths, sizes.heights, pathColors(data), upsFor(data));
      const mesh = new THREE.Mesh(geometry, colorMode === 'custom' ? partBeadMaterial(data.part) : beadMaterial);
      mesh.userData.part = data.part;
      mesh.userData.from = data.from;
      mesh.userData.upTo = upTo;
      mesh.visible = !hiddenParts.has(data.part);
      beads.add(mesh);
    }
  }

  // ---- Print progress: the toolpath up to one move, and a nozzle there ----

  /** Which part of the print is shown, as fractions of its moves: from, to (0, 1: all of it). */
  let windowFrom = 0;
  let progress = 1;
  /** While playing: how much of the move after the end is done (0..1), drawn part way. */
  let partial = 0;
  // That move part way: a line, or a short bead, from its start to where the nozzle is.
  const headLineMaterial = new LineMaterial({ linewidth: 1.1, vertexColors: true, worldUnits: false });
  const headLine = new Line2(new LineGeometry(), headLineMaterial);
  headLine.frustumCulled = false;
  const headBead = new THREE.Mesh(new THREE.BufferGeometry(), beadMaterial);
  headBead.frustumCulled = false;
  headLine.visible = headBead.visible = false;
  scene.add(headLine, headBead);
  /** Points in all the toolpaths shown, in print order. */
  let totalPoints = 0;
  const nozzle = makeNozzle();
  scene.add(nozzle);
  // The bead under the nozzle: its cross-section's outline, in a colour that stands out from
  // the toolpath's scale, drawn over everything.
  const beadEdgeMaterial = new LineMaterial({ color: 0xff2bd6, linewidth: 3, worldUnits: false, depthTest: false });
  const beadEdge = new Line2(new LineGeometry(), beadEdgeMaterial);
  beadEdge.renderOrder = 10;
  beadEdge.frustumCulled = false;
  nozzle.add(beadEdge);
  // Compiled now, so the first scroll of the progress slider doesn't stall while it compiles.
  renderer.compile(scene, camera);
  nozzle.visible = false;

  const Z_UP = new THREE.Vector3(0, 0, 1);

  /** A generic nozzle: a cone with its tip at the origin, a hex block and a heater above. */
  function makeNozzle() {
    const metal = new THREE.MeshStandardMaterial({ color: 0xc9a227, roughness: 0.35, metalness: 0.7 });
    const block = new THREE.MeshStandardMaterial({ color: 0x9aa3b2, roughness: 0.5, metalness: 0.5 });
    const tip = new THREE.Mesh(new THREE.ConeGeometry(0.5, 1, 24), metal);
    tip.rotation.x = -Math.PI / 2;
    tip.position.z = 0.5;
    const hex = new THREE.Mesh(new THREE.CylinderGeometry(0.75, 0.75, 0.6, 6), metal);
    hex.rotation.x = Math.PI / 2;
    hex.position.z = 1.3;
    const heater = new THREE.Mesh(new THREE.BoxGeometry(2.2, 1.6, 1.2), block);
    heater.position.z = 2.2;
    const g = new THREE.Group();
    g.add(tip, hex, heater);
    return g;
  }

  /**
   * Shows the toolpaths (lines and beads) from the window's start to its end; when the end is
   * short of the whole print, the nozzle is there.
   */
  function applyProgress() {
    const shown = progress >= 1 ? Infinity : Math.round(progress * totalPoints);
    const start = windowFrom <= 0 ? 0 : Math.round(windowFrom * totalPoints);
    let last = null;
    for (const line of paths.children) {
      const data = line.userData;
      const n = data.points.length / 3;
      const k = Math.max(0, Math.min(n, shown - data.from));
      const geometry = /** @type {any} */ (/** @type {Line2} */ (line).geometry);
      geometry.instanceCount = shown === Infinity ? Infinity : Math.max(0, k - 1);
      // Segment j runs from point j to j + 1: hidden while it starts before the window.
      data.clipFrom = Math.max(0, Math.min(n, start - data.from));
      if (k > 0 && shown !== Infinity && !hiddenParts.has(data.part)) last = { data, i: k - 1 };
    }
    for (const mesh of beads.children) {
      const { from, upTo } = mesh.userData;
      const n = upTo.length;
      const k = Math.max(0, Math.min(n, shown - from));
      const end = shown === Infinity || k >= n ? (upTo[n - 1] ?? 0) : k > 0 ? upTo[k - 1] : 0;
      const j = Math.max(0, Math.min(n, start - from));
      const begin = j > 0 ? Math.min(end, upTo[j - 1]) : 0;
      /** @type {THREE.Mesh} */ (mesh).geometry.setDrawRange(begin, end - begin);
    }
    nozzle.visible = Boolean(last) && mode !== 'shape';
    headLine.visible = headBead.visible = false;
    if (last) {
      const { data } = last;
      let { i } = last;
      const n = data.points.length / 3;
      // Part way along the next move: the nozzle there, and the move drawn up to it.
      if (partial > 0 && i + 1 < n) {
        const p = data.points;
        const f = Math.min(1, partial);
        const at = [0, 1, 2].map((k) => p[3 * i + k] + (p[3 * i + 3 + k] - p[3 * i + k]) * f);
        const ends = new Float32Array([p[3 * i], p[3 * i + 1], p[3 * i + 2], ...at]);
        const color = pathColors({ ...data, points: p.subarray(3 * i, 3 * i + 6), widths: data.widths?.subarray(i, i + 2), speeds: data.speeds?.subarray(i, i + 2), flows: data.flows?.subarray(i, i + 2), heights: data.heights?.subarray(i, i + 2) });
        const c1 = [color[3], color[4], color[5]];
        if (showBeads) {
          const sizes = beadSizes(data);
          const w = sizes.widths ? sizes.widths[i + 1] : 0;
          if (w > 0) {
            headBead.geometry.dispose();
            const ups = upsFor(data);
            const headUps = ups ? new Float32Array([...ups.subarray(3 * i + 3, 3 * i + 6), ...ups.subarray(3 * i + 3, 3 * i + 6)]) : null;
            headBead.geometry = beadGeometry(ends, new Float32Array([0, w]), new Float32Array([0, sizes.heights[i + 1]]), new Float32Array([...c1, ...c1]), headUps).geometry;
            headBead.visible = mode !== 'shape';
          }
        } else {
          headLine.geometry.dispose();
          const g = new LineGeometry();
          g.setPositions(ends);
          g.setColors([...c1, ...c1]);
          headLine.geometry = g;
          headLine.visible = mode !== 'shape';
        }
        i += 1;
        last.i = i;
        last.at = at;
      }
      const w = data.widths ? (data.widths[i] > 0 ? data.widths[i] : data.widths.find((v) => v > 0) || 1) : 1;
      const size = Math.max(4 * w, 0.03 * modelSize());
      nozzle.scale.setScalar(size);
      const at = last.at || [data.points[3 * i], data.points[3 * i + 1], data.points[3 * i + 2]];
      nozzle.position.set(at[0], at[1], at[2]);
      // Tilted along the wall, the bead's outline with it (it's the nozzle's child).
      const ups = upsFor(data);
      if (ups) nozzle.quaternion.setFromUnitVectors(Z_UP, new THREE.Vector3(ups[3 * i], ups[3 * i + 1], ups[3 * i + 2]));
      else nozzle.quaternion.identity();
      placeBeadEdge(data, i, size);
    }
    requestRender();
    return last;
  }

  /**
   * The outline of the bead the nozzle is laying (move `i`'s width and layer height, across its
   * direction), in the nozzle's own scaled frame. Hidden on a travel move.
   */
  function placeBeadEdge(data, i, size) {
    const sizes = beadSizes(data);
    const w = sizes.widths ? sizes.widths[i] : 0;
    const h = sizes.heights ? sizes.heights[i] : 0;
    beadEdge.visible = w > 0 && h > 0;
    if (!beadEdge.visible) return;
    const p = data.points;
    const j = i > 0 ? i - 1 : Math.min(i + 1, p.length / 3 - 1);
    let dx = p[3 * i] - p[3 * j];
    let dy = p[3 * i + 1] - p[3 * j + 1];
    if (j > i) [dx, dy] = [-dx, -dy];
    const len = Math.hypot(dx, dy) || 1;
    const [ax, ay] = [-dy / len, dx / len];
    const r = Math.min(w, h) / 2;
    const c = Math.max(0, w / 2 - r);
    const pts = [];
    const steps = 10;
    for (let k = 0; k <= 2 * steps + 1; k++) {
      const right = k <= steps;
      const a = right ? -Math.PI / 2 + (Math.PI * k) / steps : Math.PI / 2 + (Math.PI * (k - steps - 1)) / steps;
      const u = (right ? c : -c) + r * Math.cos(a);
      const v = -h / 2 + r * Math.sin(a);
      pts.push((ax * u) / size, (ay * u) / size, v / size);
    }
    pts.push(pts[0], pts[1], pts[2]);
    beadEdge.geometry.dispose();
    const g = new LineGeometry();
    g.setPositions(pts);
    beadEdge.geometry = g;
    beadEdge.computeLineDistances();
  }

  function modelSize() {
    const b = lastBounds;
    return b ? Math.hypot(b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]) : 100;
  }

  /** @param {THREE.Group} group */
  function clear(group) {
    const shared = [meshMaterial, insideMaterial, beadMaterial, lineMaterial, curveMaterial];
    for (const child of [...group.children]) {
      /** @type {any} */ (child).geometry.dispose();
      // A part's own material (custom colours) goes with it.
      const material = /** @type {any} */ (child).material;
      if (material && !shared.includes(material)) material.dispose();
      group.remove(child);
    }
  }

  /** Shows or hides each piece by its part (`userData.part`, -1: none). */
  function applyParts() {
    for (const group of [shapes, paths, curves]) {
      for (const child of group.children) child.visible = !hiddenParts.has(child.userData.part);
    }
    for (const child of beads.children) child.visible = !hiddenParts.has(child.userData.part);
    applyProgress();
  }

  function applyMode() {
    shapes.visible = mode !== 'path';
    paths.visible = mode !== 'shape' && !showBeads;
    beads.visible = mode !== 'shape' && showBeads;

    // With both visible, the shape turns see-through so the toolpath on its surface shows.
    applyCustom();
    for (const m of [meshMaterial, insideMaterial]) {
      m.transparent = mode === 'both';
      m.opacity = mode === 'both' ? 0.28 : 1;
      m.depthWrite = mode !== 'both';
      m.needsUpdate = true;
    }
    applyProgress();
  }

  /** The directions the set views look from (toward the model). Top is tipped a hair toward
   * the front, so "up" on screen stays well defined (+Y). */
  const VIEWS = {
    iso: new THREE.Vector3(0.62, -0.78, 0.5).normalize(),
    front: new THREE.Vector3(0, -1, 0),
    side: new THREE.Vector3(1, 0, 0),
    top: new THREE.Vector3(0, -1e-4, 1).normalize(),
  };

  /**
   * Frames a box: `{ min: [x, y, z], max: [x, y, z] }` (a default box when there's nothing),
   * looking from `dir`.
   */
  function fit(bounds, dir = VIEWS.iso) {
    const b = bounds || { min: [-50, -50, 0], max: [50, 50, 100] };
    const size = [0, 1, 2].map((k) => b.max[k] - b.min[k]);
    const r = Math.max(10, Math.hypot(size[0], size[1], size[2]) / 2) * 1.1;
    // The narrower of the two view angles, so it fits across a phone's narrow view too.
    const vertical = THREE.MathUtils.degToRad(persp.fov / 2);
    const half = Math.min(vertical, Math.atan(Math.tan(vertical) * aspect));
    const dist = r / Math.sin(half);
    controls.target.set((b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, (b.min[2] + b.max[2]) / 2);
    for (const c of [persp, ortho]) {
      c.position.copy(controls.target).addScaledVector(dir, dist);
      c.near = Math.max(0.5, dist / 100);
      c.far = dist * 20;
    }
    // Parallel: the same box, its narrower side across the view.
    orthoHalf = r / Math.min(1, aspect);
    ortho.zoom = 1;
    frameOrtho();
    persp.updateProjectionMatrix();
    controls.update();
    requestRender();
  }

  /** Looks at the model from one of the set views, keeping its distance and zoom. */
  function lookFrom(name) {
    const dir = VIEWS[name] || VIEWS.iso;
    const dist = camera.position.distanceTo(controls.target);
    camera.position.copy(controls.target).addScaledVector(dir, dist);
    controls.update();
    requestRender();
  }

  /** Switches between parallel and perspective, keeping what's in view the same size. */
  function setParallel(on) {
    if (on === (camera === ortho)) return;
    const target = controls.target;
    const dir = camera.position.clone().sub(target).normalize();
    if (on) {
      // The height perspective shows at the target becomes the parallel view's.
      orthoHalf = persp.position.distanceTo(target) * tanHalf();
      ortho.zoom = 1;
      ortho.position.copy(persp.position);
      ortho.near = persp.near;
      ortho.far = persp.far;
      frameOrtho();
      camera = ortho;
    } else {
      const dist = Math.max(1, orthoHalf / ortho.zoom / tanHalf());
      persp.position.copy(target).addScaledVector(dir, dist);
      persp.near = Math.max(0.5, dist / 100);
      persp.far = dist * 20;
      persp.updateProjectionMatrix();
      camera = persp;
    }
    controls.object = camera;
    controls.update();
    requestRender();
  }

  function applyTheme() {
    scene.background = new THREE.Color(token('--view-bg'));
    meshMaterial.color.set(token('--accent'));
    insideMaterial.color.set(token('--accent')).lerp(new THREE.Color(token('--view-bg')), 0.45);
    curveMaterial.color.set(token('--warn'));
    rebuildGrid();
    recolorPaths();
    requestRender();
  }
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyTheme);

  applyTheme();
  resize();

  return {
    /** WebGL 2 now; whether this device could also run WebGPU later. */
    rendererInfo() {
      const gl = renderer.capabilities.isWebGL2 ? 'WebGL 2' : 'WebGL 1';
      return `${gl}${'gpu' in navigator ? ' (WebGPU available)' : ''}`;
    },

    /** @param {'shape' | 'path' | 'both'} next */
    setMode(next) {
      mode = next;
      applyMode();
    },

    /**
     * @param {number[]} size bed width and depth in mm
     * @param {number[]} center where the recipe's origin sits on the bed, in mm
     */
    setBed(size, center) {
      bed = { size: [size[0], size[1]], center: [center[0], center[1]] };
      rebuildGrid();
      requestRender();
    },

    /**
     * Shows a new engine result: the merged mesh, curves as thin lines, toolpaths as fat lines.
     * Polylines arrive packed: `points` (x,y,z floats) and `starts` (where each one begins, in
     * points, with the total at the end). `parts` says which printed part each piece belongs to
     * (-1: none): runs of `[end, part]` over the mesh's indices, and one per curve and path.
     * @param {{ mesh: { positions: Float32Array, indices: Uint32Array },
     *   curves: { points: Float32Array, starts: Uint32Array },
     *   paths: { points: Float32Array, starts: Uint32Array },
     *   parts?: { mesh: Int32Array, curves: Int32Array, paths: Int32Array },
     *   bounds: { min: number[], max: number[] } | null }} result
     * @param {{ refit?: boolean }} [view]
     */
    show(result, view = {}) {
      const parts = result.parts || { mesh: new Int32Array(0), curves: new Int32Array(0), paths: new Int32Array(0) };
      const merged = new THREE.BufferGeometry();
      merged.setAttribute('position', new THREE.BufferAttribute(result.mesh.positions, 3));
      merged.setIndex(new THREE.BufferAttribute(result.mesh.indices, 1));
      merged.computeVertexNormals();
      // One geometry per part, sharing the merged points and normals.
      /** @type {Map<number, Uint32Array[]>} */
      const runs = new Map();
      const indices = result.mesh.indices;
      let start = 0;
      for (let k = 0; k + 1 < parts.mesh.length; k += 2) {
        const end = Math.min(parts.mesh[k], indices.length);
        if (end > start) runs.set(parts.mesh[k + 1], [...(runs.get(parts.mesh[k + 1]) || []), indices.subarray(start, end)]);
        start = Math.max(start, end);
      }
      if (start < indices.length) runs.set(-1, [...(runs.get(-1) || []), indices.subarray(start)]);
      clear(shapes);
      for (const [part, chunks] of runs) {
        const index = new Uint32Array(chunks.reduce((n, c) => n + c.length, 0));
        let at = 0;
        for (const c of chunks) {
          index.set(c, at);
          at += c.length;
        }
        const g = new THREE.BufferGeometry();
        g.setAttribute('position', merged.getAttribute('position'));
        g.setAttribute('normal', merged.getAttribute('normal'));
        g.setIndex(new THREE.BufferAttribute(index, 1));
        for (const material of [meshMaterial, insideMaterial]) {
          const m = new THREE.Mesh(g, material);
          m.userData.part = part;
          m.userData.inside = material === insideMaterial;
          shapes.add(m);
        }
      }

      lastBounds = result.bounds;
      clear(curves);
      eachPolyline(result.curves, (points, i) => {
        const g = new THREE.BufferGeometry();
        g.setAttribute('position', new THREE.BufferAttribute(points, 3));
        const line = new THREE.Line(g, curveMaterial);
        line.userData.part = parts.curves[i] ?? -1;
        curves.add(line);
      });
      clear(paths);
      const { widths, heights, speeds, flows, simWidths, simHeights, simFlags, ups } = /** @type {any} */ (result.paths);
      totalPoints = 0;
      eachPolyline(result.paths, (points, i, from) => {
        const g = new LineGeometry();
        g.setPositions(points);
        const line = new Line2(g, lineMaterial);
        const n = points.length / 3;
        const slice = (list) => (list ? list.subarray(from, from + n) : undefined);
        line.userData = {
          points,
          widths: slice(widths),
          heights: slice(heights),
          speeds: slice(speeds),
          flows: slice(flows),
          simWidths: slice(simWidths),
          simHeights: slice(simHeights),
          simFlags: slice(simFlags),
          ups: ups ? ups.subarray(3 * from, 3 * (from + n)) : undefined,
          part: parts.paths[i] ?? -1,
          from: totalPoints,
        };
        totalPoints += n;
        line.onBeforeRender = () => {
          lineMaterial.uniforms.clipFrom.value = line.userData.clipFrom || 0;
          lineMaterial.uniformsNeedUpdate = true;
        };
        paths.add(line);
      });
      recolorPaths();
      applyParts();

      applyMode();
      if (!fitted || view.refit) {
        fit(result.bounds);
        fitted = true;
      }
      requestRender();
    },

    /** Frames the last result again. */
    refit() {
      fit(lastBounds);
    },

    /**
     * Frames what's shown of the toolpath (its printing moves, between the progress handles, of
     * the parts not hidden; else the whole model) and orbits round its centre, keeping the
     * angle it's seen from: Rhino's zoom extents.
     */
    focus() {
      const shown = progress >= 1 ? Infinity : Math.round(progress * totalPoints);
      const start = windowFrom <= 0 ? 0 : Math.round(windowFrom * totalPoints);
      const lo = [Infinity, Infinity, Infinity];
      const hi = [-Infinity, -Infinity, -Infinity];
      if (mode !== 'shape') {
        for (const line of paths.children) {
          const data = line.userData;
          if (hiddenParts.has(data.part)) continue;
          const n = data.points.length / 3;
          for (let i = Math.max(0, start - data.from); i < Math.min(n, shown - data.from); i++) {
            if (data.widths && !(data.widths[i] > 0)) continue;
            for (let k = 0; k < 3; k++) {
              const v = data.points[3 * i + k];
              if (v < lo[k]) lo[k] = v;
              if (v > hi[k]) hi[k] = v;
            }
          }
        }
      }
      const bounds = Number.isFinite(lo[0]) ? { min: lo, max: hi } : lastBounds;
      const dir = camera.position.clone().sub(controls.target);
      fit(bounds, dir.lengthSq() > 0 ? dir.normalize() : VIEWS.iso);
    },

    /**
     * When each point of the toolpaths is reached, in seconds from the start of the print (in
     * print order, as `pathPoints` counts them): each printing move at its own speed, travels
     * at `travelSpeed` (mm/s). For playing the print back.
     * @param {number} travelSpeed
     */
    moveTimes(travelSpeed) {
      const out = new Float64Array(totalPoints);
      let t = 0;
      for (const line of paths.children) {
        const { points: p, speeds, from } = line.userData;
        const n = p.length / 3;
        for (let i = 0; i < n; i++) {
          if (i > 0) {
            const d = Math.hypot(p[3 * i] - p[3 * i - 3], p[3 * i + 1] - p[3 * i - 2], p[3 * i + 2] - p[3 * i - 1]);
            const v = speeds && speeds[i] > 0 ? Math.min(speeds[i], travelSpeed) : travelSpeed;
            t += d / Math.max(0.1, v);
          }
          out[from + i] = t;
        }
      }
      return out;
    },

    /**
     * Frames the model from a set view.
     * @param {'iso' | 'front' | 'side' | 'top'} name
     */
    view(name) {
      fit(lastBounds, VIEWS[name] || VIEWS.iso);
    },

    /** Parallel (orthographic) or perspective. */
    setParallel,

    /**
     * Toolpaths as solid beads at their real line width and layer height (on), or as lines.
     * @param {boolean} on
     */
    setBeads(on) {
      showBeads = on;
      if (on) buildBeads();
      else clear(beads);
      applyMode();
    },

    /**
     * How solid the beads are drawn: 1 solid, less see-through.
     * @param {number} opacity 0.1 to 1
     */
    setBeadOpacity(opacity) {
      const o = Math.min(1, Math.max(0.1, opacity));
      beadMaterial.opacity = o;
      beadMaterial.transparent = o < 1;
      beadMaterial.depthWrite = o >= 1;
      beadMaterial.needsUpdate = true;
      requestRender();
    },

    get parallel() {
      return camera === ortho;
    },

    /**
     * Draws the beads as the simulation found them (how the plastic would actually lie), or as
     * planned. The result shown has to carry the simulation (`paths.simWidths`).
     * @param {boolean} on
     */
    setSimulated(on) {
      simulated = on;
      if (showBeads) buildBeads();
      applyProgress();
    },

    /**
     * Which way the nozzle and the beads stand: 'upright', 'tilt' (along the wall, at most 45°)
     * or 'wall' (along the wall). Preview only.
     * @param {'upright' | 'tilt' | 'wall'} next
     */
    setOrientation(next) {
      orientation = ['upright', 'tilt', 'wall'].includes(next) ? next : 'upright';
      if (showBeads) buildBeads();
      applyProgress();
    },

    /** What the toolpath can be coloured by, with labels and units. */
    colorModes: COLOR_MODES,

    /**
     * Colours the toolpath (lines and beads) by its height, speed, flow, line width or layer
     * height.
     * @param {keyof typeof COLOR_MODES} next
     */
    setColorMode(next) {
      colorMode = next in COLOR_MODES ? next : 'height';
      recolorPaths();
      if (showBeads) buildBeads();
      applyMode();
    },

    /**
     * The custom colours (colour mode "custom"): each printed part's colour (`#rrggbb`) and how
     * solid it is (0..1), by its index among the prints; the highlights' white and the shadows'
     * black, 0..1.
     * @param {{ parts: { color: string, opacity: number }[], light: number, shadow: number }} next
     */
    setCustomColors(next) {
      custom = { parts: next.parts.map((p) => ({ color: p.color, opacity: Math.min(1, Math.max(0.05, p.opacity)) })), light: next.light, shadow: next.shadow };
      if (colorMode !== 'custom') return;
      recolorPaths();
      if (showBeads) buildBeads();
      applyMode();
    },

    /** The legend: the colour mode, its range and unit, and the colours from low to high. */
    legend() {
      const m = COLOR_MODES[colorMode];
      const stops = SCALE;
      const set = ranges[colorMode] || [null, null];
      return {
        mode: colorMode,
        label: m.label,
        unit: m.unit,
        digits: m.digits,
        min: range[0],
        max: range[1],
        auto: autoRange,
        set: set[0] != null || set[1] != null,
        colors: stops.map((c) => `#${c.getHexString()}`),
      };
    },

    /**
     * Sets the colour range of a colour mode by hand: the full colour scale runs from `low` to
     * `high`, and anything beyond is the colour at that end. null: automatic (the range of every
     * printing move).
     * @param {string} which a colour mode
     * @param {number | null} low
     * @param {number | null} high
     */
    setColorRange(which, low, high) {
      const ok = (v) => (v != null && Number.isFinite(v) ? v : null);
      if (ok(low) == null && ok(high) == null) delete ranges[which];
      else ranges[which] = [ok(low), ok(high)];
      if (which === colorMode) recolorPaths();
    },

    /**
     * Shows the print from one fraction of its moves to another (0 to 1: all of it), with a
     * nozzle at the end when it's short of the whole print. Returns where the nozzle is: the
     * point and its move's values, or null.
     * @param {number} f the end, 0 to 1
     * @param {number} [from] the start, 0 to 1
     * @param {number} [part] how much of the move after the end is done (playing back), 0 to 1
     */
    setProgress(f, from = 0, part = 0) {
      progress = Math.min(1, Math.max(0, f));
      partial = progress < 1 ? Math.min(1, Math.max(0, part)) : 0;
      windowFrom = Math.min(progress, Math.max(0, from));
      const last = applyProgress();
      if (!last) return null;
      const { data, i } = last;
      const pick = (list) => (list ? list[i] : 0);
      const at = last.at || [data.points[3 * i], data.points[3 * i + 1], data.points[3 * i + 2]];
      return {
        x: at[0],
        y: at[1],
        z: at[2],
        travel: Boolean(data.widths) && !(data.widths[i] > 0),
        speed: pick(data.speeds),
        flow: pick(data.flows),
        width: pick(data.widths),
        layer: pick(data.heights),
        simWidth: data.simWidths ? data.simWidths[i] : null,
        simLayer: data.simHeights ? data.simHeights[i] : null,
        simFlags: data.simFlags ? data.simFlags[i] : 0,
      };
    },

    /** How many points the toolpaths have, in print order (for the progress slider's steps). */
    get pathPoints() {
      return totalPoints;
    },

    /**
     * Hides these printed parts (indices into the result's prints) with the shapes only they are
     * made from; shows the rest.
     * @param {number[]} indices
     */
    setHiddenParts(indices) {
      hiddenParts = new Set(indices);
      applyParts();
    },

    fit,
    lookFrom,

    /**
     * Renders four views into one picture (for the agent's `look` tool): perspective, then front
     * (looking along +Y), right side (looking along -X) and top (looking down, +Y up) as
     * orthographic views, true to proportion like a technical drawing. The view on screen is left
     * as it was. Returns a JPEG data URL.
     * @param {'shape' | 'toolpath' | 'both'} [show]
     * @param {number} [size] pixels per view
     */
    snapshot(show = 'both', size = 384) {
      const saved = {
        mode,
        position: persp.position.clone(),
        up: persp.up.clone(),
        target: controls.target.clone(),
        near: persp.near,
        far: persp.far,
        pixelRatio: renderer.getPixelRatio(),
        size: renderer.getSize(new THREE.Vector2()),
      };
      mode = show === 'toolpath' ? 'path' : show === 'shape' ? 'shape' : 'both';
      applyMode();
      // The AI sees the whole model: every part, the whole print, no nozzle.
      const hidden = hiddenParts;
      const shownProgress = progress;
      const shownFrom = windowFrom;
      windowFrom = 0;
      hiddenParts = new Set();
      progress = 1;
      applyParts();
      renderer.setPixelRatio(1);
      renderer.setSize(size, size, false);
      persp.aspect = 1;
      lineMaterial.resolution.set(size, size);

      const b = lastBounds || { min: [-50, -50, 0], max: [50, 50, 100] };
      const center = new THREE.Vector3((b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, (b.min[2] + b.max[2]) / 2);
      const radius = Math.max(10, Math.hypot(b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]) / 2) * 1.08;
      const dist = radius / Math.sin(THREE.MathUtils.degToRad(persp.fov / 2));
      const flat = new THREE.OrthographicCamera(-radius, radius, radius, -radius, dist / 100, dist * 20);
      const views = [
        { label: 'perspective', cam: persp, dir: VIEWS.iso, up: new THREE.Vector3(0, 0, 1) },
        { label: 'front (looking along +Y)', cam: flat, dir: new THREE.Vector3(0, -1, 0), up: new THREE.Vector3(0, 0, 1) },
        { label: 'right (looking along -X)', cam: flat, dir: new THREE.Vector3(1, 0, 0), up: new THREE.Vector3(0, 0, 1) },
        { label: 'top (+Y up)', cam: flat, dir: new THREE.Vector3(0, 0, 1), up: new THREE.Vector3(0, 1, 0) },
      ];
      const out = document.createElement('canvas');
      out.width = out.height = size * 2;
      const ctx = /** @type {CanvasRenderingContext2D} */ (out.getContext('2d'));
      ctx.font = '13px sans-serif';
      views.forEach((v, i) => {
        const cam = v.cam;
        cam.up.copy(v.up);
        cam.position.copy(center).addScaledVector(v.dir, dist);
        cam.near = Math.max(0.5, dist / 100);
        cam.far = dist * 20;
        cam.lookAt(center);
        cam.updateProjectionMatrix();
        keyLight.position.copy(cam.position).add(new THREE.Vector3(-0.3, 0.2, 0.6).multiplyScalar(cam.position.length()));
        keyLight.target.position.copy(center);
        renderer.render(scene, cam);
        // Copy right after rendering, while the WebGL drawing buffer still holds this frame.
        const [x, y] = [(i % 2) * size, Math.floor(i / 2) * size];
        ctx.drawImage(renderer.domElement, x, y, size, size);
        ctx.fillStyle = 'rgba(0, 0, 0, 0.55)';
        ctx.fillRect(x, y, ctx.measureText(v.label).width + 16, 22);
        ctx.fillStyle = '#ffffff';
        ctx.fillText(v.label, x + 8, y + 15);
      });
      ctx.strokeStyle = 'rgba(255, 255, 255, 0.35)';
      ctx.beginPath();
      ctx.moveTo(size, 0);
      ctx.lineTo(size, size * 2);
      ctx.moveTo(0, size);
      ctx.lineTo(size * 2, size);
      ctx.stroke();

      // Back to the view on screen.
      hiddenParts = hidden;
      progress = shownProgress;
      windowFrom = shownFrom;
      applyParts();
      mode = saved.mode;
      persp.up.copy(saved.up);
      persp.position.copy(saved.position);
      persp.near = saved.near;
      persp.far = saved.far;
      controls.target.copy(saved.target);
      renderer.setPixelRatio(saved.pixelRatio);
      resize();
      applyMode();
      controls.update();
      return out.toDataURL('image/jpeg', 0.85);
    },
  };
}

/**
 * Calls `fn` with each polyline of a packed set (see `show`), its place in the set and the index
 * of its first point, skipping single points.
 * @param {{ points: Float32Array, starts: Uint32Array }} packed
 * @param {(points: Float32Array, i: number, from: number) => void} fn
 */
function eachPolyline(packed, fn) {
  for (let i = 0; i + 1 < packed.starts.length; i++) {
    const [a, b] = [packed.starts[i], packed.starts[i + 1]];
    if (b - a >= 2) fn(packed.points.subarray(a * 3, b * 3), i, a);
  }
}
