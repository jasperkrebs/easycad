// The Render view: the lamp path traced (three-gpu-pathtracer, on the GPU through WebGL 2), so
// the bulb's light bounces off solid shades, passes through see-through ones as much as they let
// it, and bends through the beads' round faces (the layer lines). Loaded only when the view is
// first opened: the rest of the app never pays for it.
//
// Two steps, as an offline renderer works: prepare (the toolpaths swept into solid beads, the
// BVH built: slow, once) and render (samples added until the set number). Preparing takes a
// snapshot: changing the project afterwards changes nothing here until it's prepared again. The
// light, exposure and the rest only restart the sampling.
//
// The scene is in metres (the recipe's mm × 0.001, so the bulb's light falls off as it does in a
// room), y up, the project turned over when it hangs the other way. The background stays
// transparent: the PNG goes on top of an environment afterwards.

import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { GradientEquirectTexture, WebGLPathTracer } from 'three-gpu-pathtracer';
import { beadGeometry } from './viewport.js';
import { touching } from './wall.js';
import { fixtureGroup } from './fixture-mesh.js';
import { CAP_SHOWS, RENDER_DEFAULTS, bulbCentre, kelvinColor } from './fixture.js';

/** The recipe's millimetres in the scene's metres. */
const MM = 0.001;
/** A 60 W bulb's 806 lm make this much light (the scene's units) at the shade. */
const UNIT_INTENSITY = 0.03;
/** Printed plastic (PLA and PETG are about 1.46-1.57). */
const IOR = 1.5;

/**
 * @typedef {{
 *   paths: { points: Float32Array, widths: Float32Array, heights: Float32Array, ups: Float32Array | null, part: number }[],
 *   looks: { color: string, opacity: number }[],
 *   fixture: ReturnType<typeof import('./fixture.js').fixtureOf>,
 * }} Snapshot
 */

/**
 * @param {HTMLCanvasElement} canvas
 * @param {{ onProgress?: (samples: number, target: number) => void }} [hooks]
 */
export function createRenderer(canvas, hooks = {}) {
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: false, alpha: true, preserveDrawingBuffer: true });
  renderer.setClearColor(0x000000, 0);
  renderer.toneMapping = THREE.AgXToneMapping;
  renderer.outputColorSpace = THREE.SRGBColorSpace;

  const camera = new THREE.PerspectiveCamera(30, 1, 0.01, 100);
  const controls = new OrbitControls(camera, canvas);
  controls.enableDamping = false;

  const scene = new THREE.Scene();
  scene.background = null;
  // The light round the lamp: a bright sky over a darker floor.
  const sky = new GradientEquirectTexture(256);
  sky.topColor.set(0xffffff);
  sky.bottomColor.set(0x5f5b56);
  sky.exponent = 1.5;
  sky.update();
  scene.environment = sky;

  const tracer = new WebGLPathTracer(renderer);
  tracer.renderDelay = 0;
  tracer.fadeDuration = 0;
  tracer.minSamples = 1;
  tracer.rasterizeScene = true;
  tracer.dynamicLowRes = true;
  // A phone's GPU gets its work in tiles, so the page stays responsive between them.
  const small = Math.min(screen.width, screen.height) < 700;
  if (small) tracer.tiles.set(3, 3);

  let settings = { ...RENDER_DEFAULTS };
  let prepared = false;
  let frame = 0;

  // Materials: the printed parts' (by part), and the fittings'.
  /** @type {Map<number, THREE.MeshPhysicalMaterial[]>} */
  const partMaterials = new Map();
  const bulbMaterial = new THREE.MeshPhysicalMaterial({ color: 0xffffff, roughness: 0.9, transmission: 1, ior: 1.5, thickness: 0, side: THREE.DoubleSide });
  // The point light inside lights the scene; the frosted glass round it mustn't shade it.
  /** @type {any} */ (bulbMaterial).castShadow = false;
  const fittings = {
    socket: new THREE.MeshPhysicalMaterial({ color: 0x2e3035, roughness: 0.45, metalness: 0, side: THREE.DoubleSide }),
    cap: new THREE.MeshPhysicalMaterial({ color: 0xd6d6d6, roughness: 0.3, metalness: 1, side: THREE.DoubleSide }),
    bulb: bulbMaterial,
    cable: new THREE.MeshPhysicalMaterial({ color: 0x1a1b1d, roughness: 0.85, metalness: 0, side: THREE.DoubleSide }),
  };
  const bulbLight = new THREE.PointLight(0xffffff, 0, 0, 2);
  /** The bulb's glass radius (m), for its glow. */
  let glassR = 0.03;

  /** Everything prepared: y up, in metres, turned over when the project hangs the other way. */
  let root = new THREE.Group();
  scene.add(root);

  function dispose(group) {
    group.traverse((o) => /** @type {any} */ (o).geometry?.dispose());
    for (const pair of partMaterials.values()) for (const m of pair) m.dispose();
    partMaterials.clear();
  }

  function resize() {
    const w = Math.max(1, canvas.clientWidth);
    const h = Math.max(1, canvas.clientHeight);
    const ratio = Math.min((window.devicePixelRatio || 1) * settings.scale, 4096 / w, 4096 / h);
    renderer.setPixelRatio(ratio);
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    if (prepared) {
      tracer.updateCamera();
      wake();
    }
  }
  const sizes = new ResizeObserver(resize);
  sizes.observe(canvas);

  controls.addEventListener('change', () => {
    if (!prepared) return;
    tracer.updateCamera();
    wake();
  });

  /** Renders samples until there are enough (then stops: the picture stays on the canvas). */
  function tick() {
    frame = 0;
    if (!prepared) return;
    tracer.renderSample();
    hooks.onProgress?.(Math.floor(tracer.samples), settings.samples);
    if (tracer.samples < settings.samples) frame = requestAnimationFrame(tick);
  }
  function wake() {
    if (!frame) frame = requestAnimationFrame(tick);
  }

  /**
   * A printed part's materials: its beads' faces (its colour, as see-through as it's set), and
   * the faces where one turn sits on the next, inside the plastic: light crosses them unbent
   * and unreflected (the same index both sides), tinted as it goes, so the wall is one solid.
   */
  function partMaterials2(look) {
    const outer = partMaterial(look);
    const inner = new THREE.MeshPhysicalMaterial({
      color: 0xffffff,
      roughness: 0,
      metalness: 0,
      transmission: 1,
      ior: 1,
      thickness: 1,
      attenuationColor: outer.attenuationColor,
      attenuationDistance: outer.attenuationDistance,
    });
    return [outer, inner];
  }

  /** A printed part's beads: its colour, and as see-through (transmissive) as it's set. */
  function partMaterial(look) {
    const color = new THREE.Color(look.color);
    const clear = Math.min(1, Math.max(0, 1 - look.opacity));
    const m = new THREE.MeshPhysicalMaterial({
      color,
      roughness: settings.matte,
      metalness: 0,
      transmission: clear,
      ior: IOR,
      // Solid: light refracts into the bead and out again, tinted by the plastic over a few mm.
      thickness: clear > 0 ? 1 : 0,
      attenuationColor: color,
      attenuationDistance: clear > 0 ? 0.005 + 0.05 * clear : Infinity,
    });
    return m;
  }

  /** The settings into the scene: the bulb, the room's light, exposure, materials, sampling. */
  function apply() {
    const [r, g, b] = kelvinColor(settings.kelvin);
    const warm = new THREE.Color(r, g, b);
    const on = settings.light;
    const intensity = (UNIT_INTENSITY * settings.lumens) / 806;
    bulbLight.color.copy(warm);
    bulbLight.intensity = on ? intensity : 0;
    // The glass glows as bright as the light it lets out: I ÷ (π r²) for a sphere.
    bulbMaterial.emissive.copy(on ? warm : new THREE.Color(0));
    bulbMaterial.emissiveIntensity = on ? intensity / (Math.PI * glassR * glassR) : 0;
    scene.environmentIntensity = on ? settings.room : 1;
    renderer.toneMappingExposure = 2 ** settings.exposure;
    tracer.bounces = settings.bounces;
    tracer.transmissiveBounces = Math.max(settings.bounces, 10);
    for (const [outer] of partMaterials.values()) outer.roughness = settings.matte;
    if (prepared) {
      tracer.updateMaterials();
      tracer.updateLights();
      tracer.updateEnvironment();
      tracer.reset();
      wake();
    }
  }

  /** Looks at the lamp (the shades, holder and bulb; not the cable) from `dir`. */
  function frame3(dir) {
    const box = new THREE.Box3();
    root.updateMatrixWorld(true);
    root.traverse((o) => {
      if (/** @type {any} */ (o).isMesh && o.name !== 'cable') box.expandByObject(o);
    });
    if (box.isEmpty()) box.set(new THREE.Vector3(-0.1, -0.1, -0.1), new THREE.Vector3(0.1, 0.1, 0.1));
    const centre = box.getCenter(new THREE.Vector3());
    const radius = Math.max(0.02, box.getSize(new THREE.Vector3()).length() / 2);
    const half = THREE.MathUtils.degToRad(camera.fov / 2);
    const fitHalf = Math.min(half, Math.atan(Math.tan(half) * camera.aspect));
    const dist = (radius / Math.sin(fitHalf)) * 0.85;
    controls.target.copy(centre);
    camera.position.copy(centre).addScaledVector(dir.clone().normalize(), dist);
    camera.near = dist / 100;
    camera.far = dist * 100;
    camera.updateProjectionMatrix();
    controls.update();
  }

  return {
    /**
     * Prepares a snapshot for rendering: every toolpath swept into solid beads, the fittings,
     * the BVH. Slow; `onStep` says what it's doing.
     * @param {Snapshot} snap
     * @param {number[]} viewDir where the 3D view looks from (recipe's x, y, z, from the target)
     * @param {(what: string) => void} [onStep]
     */
    async prepare(snap, viewDir, onStep = () => {}) {
      prepared = false;
      if (frame) cancelAnimationFrame(frame);
      frame = 0;
      scene.remove(root);
      dispose(root);
      root = new THREE.Group();
      // Recipe (z up, mm) to scene (y up, m); hung the other way: turned over first.
      root.rotation.x = -Math.PI / 2;
      root.scale.setScalar(MM);
      const hung = new THREE.Group();
      if (snap.fixture?.flipped) hung.rotation.x = Math.PI;
      root.add(hung);
      scene.add(root);
      const nextFrame = () => new Promise((resolve) => requestAnimationFrame(() => resolve(undefined)));

      let triangles = 0;
      for (let k = 0; k < snap.paths.length; k++) {
        onStep(`Beads ${k + 1}/${snap.paths.length}`);
        await nextFrame();
        const p = snap.paths[k];
        const n = p.points.length / 3;
        const white = new Float32Array(3 * n).fill(1);
        const { geometry, kinds, at } = beadGeometry(p.points, p.widths, p.heights, white, p.ups, { kinds: true });
        // The bead sweep winds its faces inward; the path tracer reads in and out from them.
        const index = geometry.getIndex();
        if (!index || !index.count) {
          geometry.dispose();
          continue;
        }
        const ix = /** @type {Uint16Array | Uint32Array} */ (index.array);
        for (let t = 0; t + 2 < ix.length; t += 3) [ix[t + 1], ix[t + 2]] = [ix[t + 2], ix[t + 1]];
        geometry.deleteAttribute('color');
        triangles += ix.length / 3;
        if (!partMaterials.has(p.part)) partMaterials.set(p.part, partMaterials2(snap.looks[p.part] || { color: '#f2f2f2', opacity: 1 }));
        // The flat faces where a turn sits on the next go inside the solid (their own group).
        const { above, below } = touching(p.points, p.widths, p.heights);
        const outerIx = [];
        const innerIx = [];
        for (let t = 0; t < ix.length / 3; t++) {
          const kind = kinds ? kinds[t] : 0;
          const i = at ? at[t] : 0;
          const inside = (kind === 1 && above[i]) || (kind === 2 && below[i]);
          (inside ? innerIx : outerIx).push(ix[3 * t], ix[3 * t + 1], ix[3 * t + 2]);
        }
        const big = geometry.getAttribute('position').count > 65535;
        const sorted = big ? new Uint32Array(ix.length) : new Uint16Array(ix.length);
        sorted.set(outerIx);
        sorted.set(innerIx, outerIx.length);
        geometry.setIndex(new THREE.BufferAttribute(sorted, 1));
        geometry.clearGroups();
        geometry.addGroup(0, outerIx.length, 0);
        if (innerIx.length) geometry.addGroup(outerIx.length, innerIx.length, 1);
        geometry.computeVertexNormals();
        const mesh = new THREE.Mesh(geometry, partMaterials.get(p.part));
        mesh.name = 'part';
        hung.add(mesh);
      }
      if (snap.fixture) {
        const fx = snap.fixture;
        const group = fixtureGroup(fx, fittings, 4000);
        glassR = (fx.glass / 2) * MM;
        bulbLight.position.set(0, 0, -(CAP_SHOWS + bulbCentre(fx)));
        group.add(bulbLight);
        hung.add(group);
      }
      // The 3D view's angle, in the scene's frame (as the lamp hangs).
      const d = new THREE.Vector3(...viewDir);
      if (snap.fixture?.flipped) d.set(d.x, -d.y, -d.z);
      frame3(new THREE.Vector3(d.x, d.z, -d.y));
      onStep('Building the BVH');
      await nextFrame();
      tracer.setScene(scene, camera);
      prepared = true;
      apply();
      return { triangles };
    },

    /** Whether there's a prepared scene. */
    get prepared() {
      return prepared;
    },

    /** The samples rendered so far. */
    get samples() {
      return prepared ? tracer.samples : 0;
    },

    /**
     * New settings (restarts the sampling; a new scale resizes the picture).
     * @param {typeof RENDER_DEFAULTS} next
     */
    setSettings(next) {
      const rescale = next.scale !== settings.scale;
      settings = { ...next };
      if (rescale) resize();
      apply();
    },

    /** Starts or carries on sampling (after the view was hidden). */
    resume() {
      resize();
      if (prepared) wake();
    },

    /** Stops sampling (the view hidden). */
    pause() {
      if (frame) cancelAnimationFrame(frame);
      frame = 0;
    },

    /** The picture as it is now, a PNG data URL with a transparent background. */
    capture() {
      return canvas.toDataURL('image/png');
    },

    /**
     * Frees everything on the GPU (the BVH, the beads, the sampling targets) and gives up the
     * canvas's context, so the 3D view can have the memory back. The canvas can't be used again.
     */
    dispose() {
      if (frame) cancelAnimationFrame(frame);
      frame = 0;
      prepared = false;
      sizes.disconnect();
      controls.dispose();
      scene.remove(root);
      dispose(root);
      for (const m of Object.values(fittings)) m.dispose();
      sky.dispose();
      tracer.dispose();
      renderer.dispose();
      renderer.forceContextLoss();
    },
  };
}
