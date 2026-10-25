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

  scene.add(new THREE.HemisphereLight(0xffffff, 0x3a3f4a, 1.5));
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

  // The graph's selected component, drawn over everything in the highlight colour (green, as
  // Grasshopper shows a selected component's geometry).
  const hlMeshMaterial = new THREE.MeshStandardMaterial({ roughness: 0.5, transparent: true, opacity: 0.55, depthWrite: false, side: THREE.DoubleSide });
  const hlMesh = new THREE.Mesh(new THREE.BufferGeometry(), hlMeshMaterial);
  const hlLineMaterial = new LineMaterial({ linewidth: 2.4, worldUnits: false });
  const hlLines = new THREE.Group();
  hlMesh.renderOrder = 2;
  hlLines.renderOrder = 3;
  scene.add(hlMesh, hlLines);
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
    hlLineMaterial.resolution.set(w, h);
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

  function pathColors(path, zMin, zMax) {
    const low = new THREE.Color(token('--accent'));
    const high = new THREE.Color(token('--accent2'));
    const out = new Float32Array(path.length);
    const c = new THREE.Color();
    const span = zMax - zMin;
    for (let i = 0; i < path.length; i += 3) {
      c.copy(low).lerp(high, span > 0 ? (path[i + 2] - zMin) / span : 0);
      out[i] = c.r;
      out[i + 1] = c.g;
      out[i + 2] = c.b;
    }
    return out;
  }

  function recolorPaths() {
    const zMin = lastBounds ? lastBounds.min[2] : 0;
    const zMax = lastBounds ? lastBounds.max[2] : 1;
    for (const line of paths.children) {
      const geometry = /** @type {LineGeometry} */ (/** @type {Line2} */ (line).geometry);
      geometry.setColors(pathColors(/** @type {any} */ (line).userData.points, zMin, zMax));
    }
    if (showBeads) buildBeads();
  }

  /**
   * One toolpath as a solid bead: a box section (the line width across, flat in the layer; the
   * layer height down from the nozzle) swept along the path, its corners shared from one point to
   * the next so it shades like a slicer's rounded bead. Travel moves leave a gap.
   * @param {Float32Array} points
   * @param {Float32Array} widths
   * @param {Float32Array} heights
   * @param {Float32Array} colors
   */
  function beadGeometry(points, widths, heights, colors) {
    const n = points.length / 3;
    const pos = new Float32Array(n * 12);
    const col = new Float32Array(n * 12);
    // Across the path in the layer: the last good direction when a move is straight up.
    let ax = 1;
    let ay = 0;
    for (let i = 0; i < n; i++) {
      const a = Math.max(0, i - 1);
      const b = Math.min(n - 1, i + 1);
      const tx = points[3 * b] - points[3 * a];
      const ty = points[3 * b + 1] - points[3 * a + 1];
      const len = Math.hypot(tx, ty);
      if (len > 1e-9) {
        ax = -ty / len;
        ay = tx / len;
      }
      // A point after a travel takes the bead of the move that leaves it.
      const k = widths[i] > 0 || i + 1 >= n ? i : i + 1;
      const hw = widths[k] / 2;
      const h = heights[k];
      const [x, y, z] = [points[3 * i], points[3 * i + 1], points[3 * i + 2]];
      const ring = [x + ax * hw, y + ay * hw, z, x - ax * hw, y - ay * hw, z, x - ax * hw, y - ay * hw, z - h, x + ax * hw, y + ay * hw, z - h];
      pos.set(ring, 12 * i);
      for (let c = 0; c < 4; c++) col.set(colors.subarray(3 * i, 3 * i + 3), 12 * i + 3 * c);
    }
    let faces = 0;
    for (let i = 1; i < n; i++) if (widths[i] > 0) faces++;
    const index = new Uint32Array(faces * 24);
    let at = 0;
    for (let i = 0; i + 1 < n; i++) {
      if (!(widths[i + 1] > 0)) continue;
      const [p, q] = [4 * i, 4 * (i + 1)];
      for (let c = 0; c < 4; c++) {
        const d = (c + 1) % 4;
        index.set([p + c, q + c, q + d, p + c, q + d, p + d], at);
        at += 6;
      }
    }
    const g = new THREE.BufferGeometry();
    g.setAttribute('position', new THREE.BufferAttribute(pos, 3));
    g.setAttribute('color', new THREE.BufferAttribute(col, 3));
    g.setIndex(new THREE.BufferAttribute(index, 1));
    g.computeVertexNormals();
    return g;
  }

  /** The beads for every toolpath shown as lines, in the same colours and parts. */
  function buildBeads() {
    clear(beads);
    const zMin = lastBounds ? lastBounds.min[2] : 0;
    const zMax = lastBounds ? lastBounds.max[2] : 1;
    for (const line of paths.children) {
      const data = /** @type {any} */ (line).userData;
      if (!data.widths || !data.heights) continue;
      const mesh = new THREE.Mesh(beadGeometry(data.points, data.widths, data.heights, pathColors(data.points, zMin, zMax)), beadMaterial);
      mesh.userData.part = data.part;
      mesh.visible = !hiddenParts.has(data.part);
      beads.add(mesh);
    }
  }

  /** @param {THREE.Group} group */
  function clear(group) {
    for (const child of [...group.children]) {
      /** @type {any} */ (child).geometry.dispose();
      group.remove(child);
    }
  }

  /** Shows or hides each piece by its part (`userData.part`, -1: none). */
  function applyParts() {
    for (const group of [shapes, paths, curves]) {
      for (const child of group.children) child.visible = !hiddenParts.has(child.userData.part);
    }
    for (const child of beads.children) child.visible = !hiddenParts.has(child.userData.part);
    requestRender();
  }

  function applyMode() {
    shapes.visible = mode !== 'path';
    paths.visible = mode !== 'shape' && !showBeads;
    beads.visible = mode !== 'shape' && showBeads;
    // With both visible, the shape turns see-through so the toolpath on its surface shows.
    for (const m of [meshMaterial, insideMaterial]) {
      m.transparent = mode === 'both';
      m.opacity = mode === 'both' ? 0.28 : 1;
      m.depthWrite = mode !== 'both';
      m.needsUpdate = true;
    }
    requestRender();
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
    hlMeshMaterial.color.set(token('--accent2'));
    hlLineMaterial.color.set(token('--accent2'));
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
      const { widths, heights } = /** @type {any} */ (result.paths);
      eachPolyline(result.paths, (points, i, from) => {
        const g = new LineGeometry();
        g.setPositions(points);
        const line = new Line2(g, lineMaterial);
        line.userData.points = points;
        if (widths && heights) {
          line.userData.widths = widths.subarray(from, from + points.length / 3);
          line.userData.heights = heights.subarray(from, from + points.length / 3);
        }
        line.userData.part = parts.paths[i] ?? -1;
        paths.add(line);
      });
      recolorPaths();
      applyParts();

      const hl = result.highlight;
      const hlGeometry = new THREE.BufferGeometry();
      if (hl && hl.mesh.indices.length) {
        hlGeometry.setAttribute('position', new THREE.BufferAttribute(hl.mesh.positions, 3));
        hlGeometry.setIndex(new THREE.BufferAttribute(hl.mesh.indices, 1));
        hlGeometry.computeVertexNormals();
      }
      hlMesh.geometry.dispose();
      hlMesh.geometry = hlGeometry;
      clear(hlLines);
      if (hl) {
        for (const packed of [hl.curves, hl.paths]) {
          eachPolyline(packed, (points) => {
            const g = new LineGeometry();
            g.setPositions(points);
            hlLines.add(new Line2(g, hlLineMaterial));
          });
        }
      }

      applyMode();
      if (!fitted || view.refit) {
        fit(result.bounds);
        fitted = true;
      }
      requestRender();
    },

    /** Frames the last result again (double-tap / double-click). */
    refit() {
      fit(lastBounds);
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

    get parallel() {
      return camera === ortho;
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
      // The AI sees the whole model: every part, and not what's selected in the graph.
      const hidden = hiddenParts;
      hiddenParts = new Set();
      applyParts();
      hlMesh.visible = hlLines.visible = false;
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
      hlMesh.visible = hlLines.visible = true;
      hiddenParts = hidden;
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
