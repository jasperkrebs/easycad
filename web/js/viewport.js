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

  const lineMaterial = new LineMaterial({ linewidth: 1.1, vertexColors: true, worldUnits: false });
  /** @type {Line2 | null} */
  let line = null;
  /** @type {THREE.GridHelper | null} */
  let grid = null;

  let mode = 'both';
  let bedSize = 256;
  let fitted = false;
  /** @type {{ path: Float32Array, height: number } | null} */
  let lastPath = null;
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
    grid = new THREE.GridHelper(bedSize, 8, c, c);
    grid.rotation.x = Math.PI / 2; // GridHelper lies in XZ; our bed is the XY plane.
    scene.add(grid);
  }

  function pathColors(path, height) {
    const low = new THREE.Color(token('--accent'));
    const high = new THREE.Color(token('--accent2'));
    const out = new Float32Array(path.length);
    const c = new THREE.Color();
    for (let i = 0; i < path.length; i += 3) {
      c.copy(low).lerp(high, height > 0 ? path[i + 2] / height : 0);
      out[i] = c.r;
      out[i + 1] = c.g;
      out[i + 2] = c.b;
    }
    return out;
  }

  function applyMode() {
    mesh.visible = mode !== 'path';
    if (line) line.visible = mode !== 'shape';
    // With both visible, the shape turns see-through so the toolpath on its surface shows.
    meshMaterial.transparent = mode === 'both';
    meshMaterial.opacity = mode === 'both' ? 0.28 : 1;
    meshMaterial.depthWrite = mode !== 'both';
    meshMaterial.needsUpdate = true;
    requestRender();
  }

  function fit(radius, height) {
    const r = Math.hypot(radius, height / 2) * 1.15;
    const dist = r / Math.sin(THREE.MathUtils.degToRad(camera.fov / 2));
    controls.target.set(0, 0, height / 2);
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
    rebuildGrid();
    if (lastPath && line) {
      /** @type {LineGeometry} */ (line.geometry).setColors(pathColors(lastPath.path, lastPath.height));
    }
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

    /** @param {number} size bed width in mm (square) */
    setBed(size) {
      bedSize = size;
      rebuildGrid();
      requestRender();
    },

    /**
     * Shows a new engine result.
     * @param {{ positions: Float32Array, indices: Uint32Array, path: Float32Array }} result
     * @param {{ radius: number, height: number, refit?: boolean }} view
     */
    show(result, view) {
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute('position', new THREE.BufferAttribute(result.positions, 3));
      geometry.setIndex(new THREE.BufferAttribute(result.indices, 1));
      geometry.computeVertexNormals();
      mesh.geometry.dispose();
      mesh.geometry = geometry;

      const lineGeometry = new LineGeometry();
      lineGeometry.setPositions(result.path);
      lineGeometry.setColors(pathColors(result.path, view.height));
      if (line) {
        line.geometry.dispose();
        line.geometry = lineGeometry;
      } else {
        line = new Line2(lineGeometry, lineMaterial);
        scene.add(line);
      }
      lastPath = { path: result.path, height: view.height };

      applyMode();
      if (!fitted || view.refit) {
        fit(view.radius, view.height);
        fitted = true;
      }
      requestRender();
    },

    /** Frames the model again (double-tap / double-click). */
    fit,
  };
}
