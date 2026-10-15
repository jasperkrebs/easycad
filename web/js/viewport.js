// 3D view: three.js drawing on the GPU (WebGL 2 today; three's WebGPURenderer is the drop-in
// upgrade when we need GPU compute or multi-million-segment toolpaths). Z is up, like Rhino and
// slicers. It renders only when something changes, which saves battery on phones.

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
  const camera = new THREE.PerspectiveCamera(35, 1, 1, 20000);
  camera.up.set(0, 0, 1);
  camera.position.set(300, -380, 260);

  scene.add(new THREE.HemisphereLight(0xffffff, 0x3a3f4a, 1.5));
  const keyLight = new THREE.DirectionalLight(0xffffff, 1.6);
  scene.add(keyLight, keyLight.target);

  const controls = new OrbitControls(camera, canvas);
  controls.addEventListener('change', requestRender);

  const meshMaterial = new THREE.MeshStandardMaterial({ roughness: 0.55, metalness: 0.0 });
  const mesh = new THREE.Mesh(new THREE.BufferGeometry(), meshMaterial);
  scene.add(mesh);

  // Toolpaths: fat lines coloured by height. Curves: thin lines.
  const lineMaterial = new LineMaterial({ linewidth: 1.1, vertexColors: true, worldUnits: false });
  const paths = new THREE.Group();
  const curveMaterial = new THREE.LineBasicMaterial();
  const curves = new THREE.Group();
  scene.add(paths, curves);

  // The graph's selected component, drawn over everything in the highlight colour (green, as
  // Grasshopper shows a selected component's geometry).
  const hlMeshMaterial = new THREE.MeshStandardMaterial({ roughness: 0.5, transparent: true, opacity: 0.55, depthWrite: false });
  const hlMesh = new THREE.Mesh(new THREE.BufferGeometry(), hlMeshMaterial);
  const hlLineMaterial = new LineMaterial({ linewidth: 2.4, worldUnits: false });
  const hlLines = new THREE.Group();
  hlMesh.renderOrder = 2;
  hlLines.renderOrder = 3;
  scene.add(hlMesh, hlLines);
  /** @type {THREE.GridHelper | null} */
  let grid = null;

  let mode = 'both';
  /** Bed width and depth, and where the recipe's origin sits on it (all in mm). */
  let bed = { size: [256, 256], center: [125, 125] };
  let fitted = false;
  /** @type {{ min: number[], max: number[] } | null} */
  let lastBounds = null;
  let frame = 0;

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

  function resize() {
    const w = Math.max(1, canvas.clientWidth);
    const h = Math.max(1, canvas.clientHeight);
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
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
  }

  /** @param {THREE.Group} group */
  function clear(group) {
    for (const child of [...group.children]) {
      /** @type {any} */ (child).geometry.dispose();
      group.remove(child);
    }
  }

  function applyMode() {
    mesh.visible = mode !== 'path';
    paths.visible = mode !== 'shape';
    // With both visible, the shape turns see-through so the toolpath on its surface shows.
    meshMaterial.transparent = mode === 'both';
    meshMaterial.opacity = mode === 'both' ? 0.28 : 1;
    meshMaterial.depthWrite = mode !== 'both';
    meshMaterial.needsUpdate = true;
    requestRender();
  }

  /** Frames a box: `{ min: [x, y, z], max: [x, y, z] }` (a default box when there's nothing). */
  function fit(bounds) {
    const b = bounds || { min: [-50, -50, 0], max: [50, 50, 100] };
    const size = [0, 1, 2].map((k) => b.max[k] - b.min[k]);
    const r = Math.max(10, Math.hypot(size[0], size[1], size[2]) / 2) * 1.1;
    const dist = r / Math.sin(THREE.MathUtils.degToRad(camera.fov / 2));
    controls.target.set((b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, (b.min[2] + b.max[2]) / 2);
    const dir = new THREE.Vector3(0.62, -0.78, 0.5).normalize();
    camera.position.copy(controls.target).addScaledVector(dir, dist);
    camera.near = Math.max(0.5, dist / 100);
    camera.far = dist * 20;
    camera.updateProjectionMatrix();
    controls.update();
    requestRender();
  }

  function applyTheme() {
    scene.background = new THREE.Color(token('--view-bg'));
    meshMaterial.color.set(token('--accent'));
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
     * points, with the total at the end).
     * @param {{ mesh: { positions: Float32Array, indices: Uint32Array },
     *   curves: { points: Float32Array, starts: Uint32Array },
     *   paths: { points: Float32Array, starts: Uint32Array },
     *   bounds: { min: number[], max: number[] } | null }} result
     * @param {{ refit?: boolean }} [view]
     */
    show(result, view = {}) {
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute('position', new THREE.BufferAttribute(result.mesh.positions, 3));
      geometry.setIndex(new THREE.BufferAttribute(result.mesh.indices, 1));
      geometry.computeVertexNormals();
      mesh.geometry.dispose();
      mesh.geometry = geometry;

      lastBounds = result.bounds;
      clear(curves);
      eachPolyline(result.curves, (points) => {
        const g = new THREE.BufferGeometry();
        g.setAttribute('position', new THREE.BufferAttribute(points, 3));
        curves.add(new THREE.Line(g, curveMaterial));
      });
      clear(paths);
      eachPolyline(result.paths, (points) => {
        const g = new LineGeometry();
        g.setPositions(points);
        const line = new Line2(g, lineMaterial);
        line.userData.points = points;
        paths.add(line);
      });
      recolorPaths();

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

    fit,

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
        position: camera.position.clone(),
        up: camera.up.clone(),
        target: controls.target.clone(),
        near: camera.near,
        far: camera.far,
        pixelRatio: renderer.getPixelRatio(),
        size: renderer.getSize(new THREE.Vector2()),
      };
      mode = show === 'toolpath' ? 'path' : show === 'shape' ? 'shape' : 'both';
      applyMode();
      // The AI sees the model, not what's selected in the graph.
      hlMesh.visible = hlLines.visible = false;
      renderer.setPixelRatio(1);
      renderer.setSize(size, size, false);
      camera.aspect = 1;
      lineMaterial.resolution.set(size, size);

      const b = lastBounds || { min: [-50, -50, 0], max: [50, 50, 100] };
      const center = new THREE.Vector3((b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, (b.min[2] + b.max[2]) / 2);
      const radius = Math.max(10, Math.hypot(b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]) / 2) * 1.08;
      const dist = radius / Math.sin(THREE.MathUtils.degToRad(camera.fov / 2));
      const ortho = new THREE.OrthographicCamera(-radius, radius, radius, -radius, dist / 100, dist * 20);
      const views = [
        { label: 'perspective', cam: camera, dir: new THREE.Vector3(0.62, -0.78, 0.5).normalize(), up: new THREE.Vector3(0, 0, 1) },
        { label: 'front (looking along +Y)', cam: ortho, dir: new THREE.Vector3(0, -1, 0), up: new THREE.Vector3(0, 0, 1) },
        { label: 'right (looking along -X)', cam: ortho, dir: new THREE.Vector3(1, 0, 0), up: new THREE.Vector3(0, 0, 1) },
        { label: 'top (+Y up)', cam: ortho, dir: new THREE.Vector3(0, 0, 1), up: new THREE.Vector3(0, 1, 0) },
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
      mode = saved.mode;
      camera.up.copy(saved.up);
      camera.position.copy(saved.position);
      camera.near = saved.near;
      camera.far = saved.far;
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
 * Calls `fn` with each polyline of a packed set (see `show`), skipping single points.
 * @param {{ points: Float32Array, starts: Uint32Array }} packed
 * @param {(points: Float32Array) => void} fn
 */
function eachPolyline(packed, fn) {
  for (let i = 0; i + 1 < packed.starts.length; i++) {
    const [a, b] = [packed.starts[i], packed.starts[i + 1]];
    if (b - a >= 2) fn(packed.points.subarray(a * 3, b * 3));
  }
}
