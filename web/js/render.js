// The Render view: the lamp path traced (three-gpu-pathtracer, on the GPU through WebGL 2), so
// the bulb's light bounces off solid shades, passes through see-through ones as much as they let
// it, and bends through the beads' round faces (the layer lines). Loaded only when the view is
// first opened: the rest of the app never pays for it.
//
// Three steps, as an offline renderer works:
// - prepare: the toolpaths swept into solid beads, once (slow). A snapshot: changing the project
//   afterwards changes nothing here until it's prepared again;
// - set up: the light, the materials, the camera, the room. Nothing renders while you do: the
//   canvas shows a plain draft (grey, quick) to frame the picture;
// - render: Preview (small, few samples) or Render (the picture's size and samples), until done
//   or Stop. What's on the canvas then is only ever the path tracer's.
//
// The scene is in metres (the recipe's mm × 0.001, so the bulb's light falls off as it does in a
// room), y up, the project turned over when it hangs the other way, the front towards +z. The
// background stays transparent: the PNG goes on top of an environment afterwards. Walls, floor
// and ceiling, when on, are in the picture, lit by the lamp.

import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { DenoiseMaterial, GradientEquirectTexture, WebGLPathTracer } from 'three-gpu-pathtracer';
import { FullScreenQuad } from 'three/addons/postprocessing/Pass.js';
import { beadGeometry } from './viewport.js';
import { touching } from './wall.js';
import { fixtureGroup } from './fixture-mesh.js';
import { CAMERA_VIEWS, CAP_SHOWS, RENDER_DEFAULTS, bulbCentre, cleanRender, kelvinColor } from './fixture.js';
import { physicalOf } from './materials.js';
import { blendDenoised, takenDetail, tameFireflies } from './denoise-prep.js';

/**
 * The denoiser's networks (Open Image Denoise 2.3's weights for clean albedo and normal guides):
 * 'detailed' the large one (7.7 MB, slower, keeps fine detail better), 'fast' the standard one.
 */
const UNETS = { detailed: 'rt_hdr_calb_cnrm_large.tza', fast: 'rt_hdr_calb_cnrm.tza' };
/** The denoiser's settings: they change a picture already made (the rest wait for the next). */
const DENOISE_KEYS = ['denoise', 'denoiseStrength', 'denoiseDetail', 'denoiseModel'];

/** The recipe's millimetres in the scene's metres. */
const MM = 0.001;
/** A 60 W bulb's 806 lm make this much light (the scene's units) at the shade. */
const UNIT_INTENSITY = 0.03;
/** The cable's length (mm) without a ceiling: out of the picture. */
const LONG_CABLE = 4000;
/** Pixel-bounces one GPU dispatch may take (a tile): kept small so the page, and the Mac's GPU, stay responsive. */
const DISPATCH = { phone: 1.5e6, apple: 1e6, other: 5e6 };
/**
 * What one piece of GPU work should take (ms), as measured: the tiles are resized between samples
 * to keep each piece about this long. (Measured to the next frame, so 16 ms is the least seen.)
 */
const DISPATCH_MS = 30;
/** The most tiles a side (16 × 16 pieces a sample). */
const MAX_TILES = 16;
/** A Mac's (or iPhone's, iPad's) GPU resets a page whose GPU work runs long sooner than others. */
const APPLE = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent || '');
/** On a Mac, up to this many tiles a side (the pieces smaller still, when its GPU has reset). */
const MAX_TILES_APPLE = 32;
/**
 * A Mac's care: its pieces of GPU work are this many times smaller than the Mac's usual. Each
 * reset of its GPU makes them 4 × smaller (remembered in this browser), each finished picture a
 * little larger again. Other computers never use it.
 */
const CARE_KEY = 'easycad.renderCare';
const CARE_MAX = 64;
function care() {
  try {
    const c = Number(localStorage.getItem(CARE_KEY));
    return Number.isFinite(c) && c >= 1 ? Math.min(CARE_MAX, c) : 1;
  } catch {
    return 1;
  }
}
/** @param {number} c */
function setCare(c) {
  try {
    localStorage.setItem(CARE_KEY, String(Math.min(CARE_MAX, Math.max(1, c))));
  } catch {
    // no storage: the care starts at 1 each time
  }
}

/** Thrown when the browser has no working WebGL 2 context to give (`code` 'no-gl'). */
export class NoGraphicsError extends Error {
  constructor() {
    super('no graphics context right now (the graphics card was reset)');
    this.code = 'no-gl';
  }
}

/**
 * @typedef {{ color: string, opacity: number, material?: any }} Look
 * @typedef {{
 *   paths: { points: Float32Array, widths: Float32Array, heights: Float32Array, ups: Float32Array | null, part: number }[],
 *   looks: Look[],
 *   fixture: ReturnType<typeof import('./fixture.js').fixtureOf>,
 * }} Snapshot
 * @typedef {{ state: 'empty' | 'draft' | 'compiling' | 'rendering' | 'denoising' | 'done' | 'stopped', kind?: 'preview' | 'final',
 *   samples?: number, target?: number, width?: number, height?: number, seconds?: number,
 *   denoised?: 'oidn' | 'smooth', showing?: boolean, denoiseError?: string }} Progress
 */

/**
 * @param {HTMLCanvasElement} canvas
 * @param {{ onProgress?: (p: Progress) => void, onCamera?: (camera: { position: number[], target: number[] }) => void, onLost?: (doing: string) => void }} [hooks]
 */
export function createRenderer(canvas, hooks = {}) {
  // The context first, checked: after the GPU was reset (a Mac does that when a page's GPU work
  // runs long or memory runs out), the browser can hand out a context that's already lost, and
  // three.js then fails deep inside ("getShaderPrecisionFormat ... null").
  const attributes = { antialias: true, alpha: true, preserveDrawingBuffer: true, powerPreference: /** @type {const} */ ('high-performance') };
  const context = canvas.getContext('webgl2', attributes);
  if (!context || context.isContextLost()) throw new NoGraphicsError();
  const renderer = new THREE.WebGLRenderer({ canvas, context, ...attributes });
  renderer.setClearColor(0x000000, 0);
  renderer.toneMapping = THREE.AgXToneMapping;
  renderer.outputColorSpace = THREE.SRGBColorSpace;

  const camera = new THREE.PerspectiveCamera(27, 1, 0.01, 100);
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
  // Only ever the path tracer's picture on the canvas (no raster stand-in while it compiles or
  // the camera moves: that looked like another material), and the same noise every time.
  tracer.rasterizeScene = false;
  tracer.dynamicLowRes = false;
  tracer.stableNoise = true;
  const phone = Math.min(screen.width, screen.height) < 700;
  /** The context was lost (the GPU reset): nothing more is drawn; the app starts afresh. */
  let lost = false;
  canvas.addEventListener('webglcontextlost', (e) => {
    e.preventDefault();
    // A Mac whose GPU reset while rendering: smaller pieces from now on.
    if (APPLE && job) setCare(care() * 4);
    lost = true;
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
    job = null;
    hooks.onLost?.(doing());
  });

  let settings = cleanRender(RENDER_DEFAULTS);
  /** @type {Snapshot | null} */
  let snap = null;
  /** The scene the path tracer has (BVH, lights) is out of date (surfaces, cable). */
  let sceneDirty = true;
  let frame = 0;
  /** What's running: null, or the kind of render, its target, and when it started. */
  let job = /** @type {{ kind: 'preview' | 'final', target: number, t0: number } | null} */ (null);
  /** What the canvas shows. */
  let shows = /** @type {Progress['state']} */ ('empty');
  /**
   * The denoised picture: Open Image Denoise's (a float texture, as the path tracer's own), or
   * the path tracer's edge-aware smoothing where there's no WebGPU; and whether it's showing.
   * Open Image Denoise's keeps the picture as rendered and as denoised (floats), to blend by the
   * denoise strength.
   * @type {{ texture: THREE.DataTexture | null, by: 'oidn' | 'smooth', on: boolean, last: Progress, raw?: Float32Array, den?: Float32Array, taken?: Float32Array } | null}
   */
  let clean = null;
  /** The last finished or stopped picture (what Denoise works on). */
  let lastPicture = /** @type {Progress | null} */ (null);
  /** Each denoise has a number: a newer render, draft or stop makes an older one's result moot. */
  let denoiseRun = 0;
  /** @type {(() => void) | null} */
  let denoiseAbort = null;
  /** The networks, each loaded once when first used. @type {Map<string, Promise<any>>} */
  const unets = new Map();

  // Materials: the printed parts' (by part: the beads, and the faces inside the wall), the
  // fittings', the room's.
  /** @type {Map<number, THREE.MeshPhysicalMaterial[]>} */
  const partMaterials = new Map();
  // Frosted glass is opal white: solid to the eye (and in the PNG's alpha), glowing when on.
  const bulbMaterial = new THREE.MeshPhysicalMaterial({ color: 0xf4f2ee, roughness: 0.9, metalness: 0, side: THREE.DoubleSide });
  // The point light inside lights the scene; the glass round it mustn't shade it.
  /** @type {any} */ (bulbMaterial).castShadow = false;
  const fittings = {
    socket: new THREE.MeshPhysicalMaterial({ color: 0x2e3035, roughness: 0.45, metalness: 0, side: THREE.DoubleSide }),
    cap: new THREE.MeshPhysicalMaterial({ color: 0xd6d6d6, roughness: 0.3, metalness: 1, side: THREE.DoubleSide }),
    bulb: bulbMaterial,
    cable: new THREE.MeshPhysicalMaterial({ color: 0x1a1b1d, roughness: 0.85, metalness: 0, side: THREE.DoubleSide }),
  };
  // The room's surfaces are solid from the room's side only (each faces the lamp): from outside
  // (the camera above the ceiling, below the floor, behind the wall) you look through them, as a
  // cutaway. Both sides solid, they hid the lamp from such a camera (the draft, drawing front
  // faces only, still showed it).
  const surfaceMaterial = new THREE.MeshPhysicalMaterial({ color: 0xe9e5df, roughness: 0.92, metalness: 0, side: THREE.FrontSide });
  const bulbLight = new THREE.PointLight(0xffffff, 0, 0, 2);
  /** The largest the light can be where it is (smaller when the wall cuts through the bulb). */
  let lightRoom = Infinity;
  /** The bulb's glass radius (m), for its glow. */
  let glassR = 0.03;

  // The draft: the scene in plain grey, lit simply, to frame the picture without waiting.
  const draftMaterial = new THREE.MeshStandardMaterial({ color: 0xc9ced6, roughness: 0.75, metalness: 0 });
  const draftLight = new THREE.HemisphereLight(0xffffff, 0x6b6f78, 2.2);

  /** The lamp (y up, in metres, turned over when it hangs the other way); the room round it. */
  let root = new THREE.Group();
  /** @type {THREE.Group} */
  let hung = new THREE.Group();
  /** @type {THREE.Object3D | null} */
  let fixture = null;
  const room = new THREE.Group();
  scene.add(root, room);

  /** The lamp's box in the scene (shades, holder, bulb; not the cable). */
  function lampBox() {
    const box = new THREE.Box3();
    root.updateMatrixWorld(true);
    root.traverse((o) => {
      if (/** @type {any} */ (o).isMesh && o.name !== 'cable') box.expandByObject(o);
    });
    if (box.isEmpty()) box.set(new THREE.Vector3(-0.1, -0.1, -0.1), new THREE.Vector3(0.1, 0.1, 0.1));
    return box;
  }

  function disposeGroup(group) {
    group.traverse((o) => /** @type {any} */ (o).geometry?.dispose());
  }

  /** What the GPU was last given to do (said when it's reset, to tell what trips it). */
  let task = 'nothing';
  const doing = () => (job && task === 'rendering' ? `rendering (sample ${Math.floor(tracer.samples) + 1}, ${tracer.tiles.x}×${tracer.tiles.y} tiles)` : task);

  /** The last progress reported (said again when what's waiting changes). */
  let lastReport = /** @type {Progress} */ ({ state: 'empty' });
  function report(p) {
    lastReport = p;
    // A picture held (under way, done or stopped): the camera stays put; the view moves the picture.
    controls.enabled = !held();
    hooks.onProgress?.({ ...p, pending: Boolean(pendingSettings || pendingLooks) });
  }

  // ---- The picture held: once a render has begun, nothing throws it away but `discard`, a new
  // Preview or Render, or a new Prepare. Settings and looks changed meanwhile wait (`pending`)
  // and apply then; the denoiser's apply to the picture at once.

  /** Settings waiting for the next picture. @type {ReturnType<typeof cleanRender> | null} */
  let pendingSettings = null;
  /** Looks waiting for the next picture. @type {Look[] | null} */
  let pendingLooks = null;
  /** Whether the canvas holds a picture (under way, done or stopped). */
  const held = () => Boolean(job) || shows === 'rendering' || shows === 'done' || shows === 'stopped' || shows === 'compiling';

  // ---- The picture's size: the CSS box stays; the drawing buffer is the render's pixels.

  /** The canvas's size on screen (CSS pixels). */
  const box = () => [Math.max(1, canvas.clientWidth), Math.max(1, canvas.clientHeight)];

  /** Sets the drawing buffer so its longer side is `px` (the draft: the screen's, at most 1.5×). */
  function sizeFor(px) {
    const [w, h] = box();
    const ratio = px ? px / Math.max(w, h) : Math.min(window.devicePixelRatio || 1, 1.5);
    renderer.setPixelRatio(ratio);
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
  }

  let lastBox = '';
  const sizes = new ResizeObserver(() => {
    const now = box().join('x');
    if (now === lastBox) return;
    lastBox = now;
    // A picture held keeps its pixels (the page shows it whole, `object-fit`); the draft, drawn
    // again, takes the new shape.
    if (held()) return;
    draft();
  });
  sizes.observe(canvas);

  // ---- The camera: orbiting, a view to fit, a lens; where it was left goes back to the app.

  controls.addEventListener('change', () => {
    if (held()) return;
    draft();
  });
  controls.addEventListener('end', () => saveCamera());

  function saveCamera() {
    const where = { position: camera.position.toArray(), target: controls.target.toArray() };
    // Kept here too: the app saves it with the project and hands the settings back on the next
    // change; not kept here, every change (the denoise strength too) looked like a new camera,
    // and stopped the render and cleared the picture.
    settings = cleanRender({ ...settings, camera: { ...settings.camera, ...where } });
    hooks.onCamera?.(where);
  }

  function lensFov(lens) {
    return THREE.MathUtils.radToDeg(2 * Math.atan(12 / Math.max(1, lens)));
  }

  /** Frames the lamp from a view (its direction), with the lens as set. */
  function fit(view) {
    const dir = new THREE.Vector3(...(CAMERA_VIEWS[view] || CAMERA_VIEWS.front).dir).normalize();
    const b = lampBox();
    const centre = b.getCenter(new THREE.Vector3());
    const radius = Math.max(0.02, b.getSize(new THREE.Vector3()).length() / 2);
    camera.fov = lensFov(settings.camera.lens);
    const half = THREE.MathUtils.degToRad(camera.fov / 2);
    const fitHalf = Math.min(half, Math.atan(Math.tan(half) * camera.aspect));
    const dist = radius / Math.sin(fitHalf);
    controls.target.copy(centre);
    camera.position.copy(centre).addScaledVector(dir, dist);
    clip();
    controls.update();
  }

  /** Near and far planes from how far the camera is. */
  function clip() {
    const d = camera.position.distanceTo(controls.target);
    camera.near = Math.max(0.001, d / 200);
    camera.far = d * 200;
    camera.updateProjectionMatrix();
  }

  /** The camera as the settings have it: where it was left, or fitted to its view. */
  function placeCamera() {
    const c = settings.camera;
    camera.fov = lensFov(c.lens);
    if (c.position && c.target) {
      camera.position.fromArray(c.position);
      controls.target.fromArray(c.target);
      clip();
      controls.update();
    } else {
      fit(c.view);
    }
  }

  // ---- The materials.

  /** A part's two materials (its beads; the faces inside its wall) set from its look. */
  function setLook(pair, look) {
    const [outer, inner] = pair;
    inner.needsUpdate = true;
    const p = physicalOf(look);
    const color = new THREE.Color(p.color);
    outer.color.copy(color);
    outer.transmission = p.transmission;
    outer.roughness = p.roughness;
    outer.metalness = p.metalness;
    outer.ior = p.ior;
    outer.clearcoat = p.clearcoat;
    outer.clearcoatRoughness = p.clearcoatRoughness;
    outer.sheen = p.sheen;
    outer.sheenColor.copy(color);
    outer.sheenRoughness = p.sheenRoughness;
    outer.thickness = p.thickness;
    outer.attenuationColor.copy(color);
    outer.attenuationDistance = p.attenuationDistance;
    outer.needsUpdate = true;
    // The faces between turns: see-through (a thin film light crosses) in a part that lets light
    // through; in a solid one, solid too. (See-through in a solid part, they let light slip
    // through the seams where one turn's bead meets the next: rings of light inside a lamp.)
    if (p.transmission > 0) {
      inner.color.set(0xffffff);
      inner.transmission = 1;
      inner.roughness = 0;
    } else {
      inner.color.copy(color);
      inner.transmission = 0;
      inner.roughness = p.roughness;
    }
  }

  function newPair() {
    return [
      new THREE.MeshPhysicalMaterial({ side: THREE.FrontSide }),
      // Inside the wall, where a turn sits on the next: a thin film light crosses unbent. (Not a
      // face of index 1 between two solids: that blackened half the pixels, NaNs in the path
      // tracer's transmission; nor a real interface: light trapped in each turn.)
      new THREE.MeshPhysicalMaterial({ color: 0xffffff, roughness: 0, metalness: 0, transmission: 1, ior: 1.5, thickness: 0 }),
    ];
  }

  // ---- The room and the light.

  /** The fittings (their cable up to the ceiling, when there's one) and the room's surfaces. */
  function buildRoom() {
    if (fixture) {
      fixture.parent?.remove(fixture);
      disposeGroup(fixture);
      fixture = null;
    }
    disposeGroup(room);
    room.clear();
    lightRoom = Infinity;
    if (snap?.fixture) {
      const fx = snap.fixture;
      const cable = settings.ceiling.on ? Math.max(1, settings.ceiling.distance) : LONG_CABLE;
      fixture = fixtureGroup(fx, fittings, cable);
      glassR = (fx.glass / 2) * MM;
      bulbLight.position.set(0, 0, -((/** @type {any} */ (fx).bulbDrop || 0) + CAP_SHOWS + bulbCentre(fx)));
      fixture.add(bulbLight);
      hung.add(fixture);
    }
    const b = lampBox();
    // The socket's top: where the cable starts (the lamp's top without fittings).
    let top = b.max.y;
    root.updateMatrixWorld(true);
    const cable = fixture?.getObjectByName('cable');
    if (cable) top = new THREE.Box3().setFromObject(cable).min.y;
    const size = 20;
    const plane = (w, h) => new THREE.PlaneGeometry(w, h);
    if (settings.wall.on) {
      const wall = new THREE.Mesh(plane(size, size), surfaceMaterial);
      // From the lamp's axis, or from its back (the furthest it reaches behind: just behind it).
      const wallZ = (settings.wall.from === 'back' ? b.min.z : 0) - settings.wall.distance * MM;
      wall.position.set(0, (b.min.y + b.max.y) / 2, wallZ);
      wall.name = 'wall';
      room.add(wall);
      // The wall cutting through the bulb (a section through the lamp): the light comes from the
      // bulb's half in front of it, about half its radius out, not from its middle (on the wall
      // itself, the light would only graze the wall and leave it dark).
      if (fixture && bulbLight.parent === fixture) {
        root.updateMatrixWorld(true);
        const at = bulbLight.getWorldPosition(new THREE.Vector3());
        if (Math.abs(at.z - wallZ) < glassR) {
          at.z = wallZ + glassR * 0.5;
          bulbLight.position.copy(fixture.worldToLocal(at));
          lightRoom = glassR * 0.45;
        }
      }
    }
    if (settings.floor.on) {
      const floor = new THREE.Mesh(plane(size, size), surfaceMaterial);
      floor.rotation.x = -Math.PI / 2;
      floor.position.set(0, b.min.y - settings.floor.distance * MM, 0);
      floor.name = 'floor';
      room.add(floor);
    }
    if (settings.ceiling.on) {
      const ceiling = new THREE.Mesh(plane(size, size), surfaceMaterial);
      ceiling.rotation.x = Math.PI / 2;
      ceiling.position.set(0, top + settings.ceiling.distance * MM, 0);
      ceiling.name = 'ceiling';
      room.add(ceiling);
    }
    surfaceMaterial.color.set(settings.surface);
    sceneDirty = true;
  }

  /** The light and the picture's look from the settings. */
  function applyLight() {
    const [r, g, b] = kelvinColor(settings.kelvin);
    const warm = new THREE.Color(r, g, b);
    const on = settings.light;
    const intensity = (UNIT_INTENSITY * settings.lumens) / 806;
    bulbLight.color.copy(warm);
    bulbLight.intensity = intensity;
    // The light shines from the glass, not its middle: a sphere a little inside it (the cap's
    // end stays outside), or, with the wall through the bulb, inside the half in front of it.
    /** @type {any} */ (bulbLight).radius = Math.min(settings.lightSize * glassR * 0.85, lightRoom);
    // Off, the light leaves the scene, not just goes dark: a light of no power is a division by
    // zero in the path tracer's light sampling, which on some GPUs (a Mac's) makes NaNs, and a
    // pixel with a NaN stays black (see-through shades went black with the light off).
    if (on && fixture && bulbLight.parent !== fixture) fixture.add(bulbLight);
    if (!on) bulbLight.removeFromParent();
    // The glass glows as bright as the light it lets out: I ÷ (π r²) for a sphere.
    bulbMaterial.emissive.copy(on ? warm : new THREE.Color(0));
    bulbMaterial.emissiveIntensity = on ? intensity / (Math.PI * glassR * glassR) : 0;
    scene.environmentIntensity = on ? settings.room : 1;
    renderer.toneMappingExposure = 2 ** settings.exposure;
    surfaceMaterial.color.set(settings.surface);
    tracer.bounces = settings.bounces;
    // Passing through see-through plastic doesn't count against the bounces, up to this many: a
    // ray from the camera to the bulb in a shade with several walls (the Thread pendant's wall,
    // wings and rings) crosses dozens of faces. At 10, the bulb behind them came out dim (paths
    // cut off count as black); 32 gave 10-14 % more light there, 64 nothing more.
    tracer.transmissiveBounces = Math.max(settings.bounces, 32);
  }

  // ---- Drawing: the draft, and the path tracer's samples.

  /** The plain grey draft (only while nothing rendered is showing). */
  function draft() {
    if (job || lost) return;
    task = 'drawing the draft';
    clearDenoised();
    if (!snap) {
      renderer.clear();
      shows = 'empty';
      report({ state: 'empty' });
      return;
    }
    sizeFor(0);
    renderer.toneMappingExposure = 1;
    scene.overrideMaterial = draftMaterial;
    scene.add(draftLight);
    const env = scene.environment;
    scene.environment = null;
    renderer.render(scene, camera);
    scene.environment = env;
    scene.remove(draftLight);
    scene.overrideMaterial = null;
    renderer.toneMappingExposure = 2 ** settings.exposure;
    shows = 'draft';
    report({ state: 'draft' });
  }

  // ---- Pacing: one piece of GPU work at a time, each about DISPATCH_MS long. A Mac resets the
  // GPU when a page's work runs long, and Safari then keeps WebGL off for the page until it's
  // restarted: so the render never queues more work before the GPU has finished the last piece
  // (a fence tells), and resizes its tiles by how long the pieces really take (the scene, not
  // just pixels × bounces, sets that).
  const gl = /** @type {WebGL2RenderingContext} */ (renderer.getContext());
  /** @type {{ sync: WebGLSync, t0: number } | null} */
  let fence = null;
  /** The last piece's time (ms), or 0. */
  let lastMs = 0;

  /** Whether the GPU is still on the last piece (else its time is taken). */
  function gpuBusy() {
    if (!fence) return false;
    const status = gl.clientWaitSync(fence.sync, 0, 0);
    if (status === gl.TIMEOUT_EXPIRED) return true;
    lastMs = performance.now() - fence.t0;
    gl.deleteSync(fence.sync);
    fence = null;
    return false;
  }

  function clearFence() {
    if (fence) gl.deleteSync(fence.sync);
    fence = null;
    lastMs = 0;
  }

  /** Between samples (all tiles done): smaller tiles when a piece ran long, larger when short. */
  function retile() {
    if (!lastMs || Math.abs(tracer.samples - Math.round(tracer.samples)) > 1e-6) return;
    const n = tracer.tiles.x;
    let next = n;
    if (lastMs > 2 * DISPATCH_MS) next = Math.min(APPLE ? MAX_TILES_APPLE : MAX_TILES, Math.ceil(n * Math.sqrt(lastMs / DISPATCH_MS)));
    else if (lastMs < DISPATCH_MS * 0.6 && n > 1) next = n - 1;
    if (next !== n) tracer.tiles.set(next, next);
  }

  function tick() {
    frame = 0;
    if (!job || lost) return;
    if (tracer.isCompiling) {
      report({ state: 'compiling', kind: job.kind });
      task = 'compiling the shaders';
      tracer.renderSample();
      frame = requestAnimationFrame(tick);
      return;
    }
    if (gpuBusy()) {
      frame = requestAnimationFrame(tick);
      return;
    }
    retile();
    const t0 = performance.now();
    task = 'rendering';
    tracer.renderSample();
    fence = { sync: /** @type {WebGLSync} */ (gl.fenceSync(gl.SYNC_GPU_COMMANDS_COMPLETE, 0)), t0 };
    gl.flush();
    const samples = Math.floor(tracer.samples);
    const size = renderer.getDrawingBufferSize(new THREE.Vector2());
    const seconds = (performance.now() - job.t0) / 1000;
    if (samples >= job.target) {
      const kind = job.kind;
      job = null;
      shows = 'done';
      const p = /** @type {Progress} */ ({ state: 'done', kind, samples, target: samples, width: size.x, height: size.y, seconds });
      if (APPLE) setCare(care() * 0.85);
      report(p);
      lastPicture = p;
      if (settings.denoise) denoise(p);
      return;
    }
    shows = 'rendering';
    report({ state: 'rendering', kind: job.kind, samples, target: job.target, width: size.x, height: size.y, seconds });
    frame = requestAnimationFrame(tick);
  }

  /**
   * Stops rendering. `thenDenoise`: the picture so far is kept (Stop pressed), so it's denoised
   * as a finished one would be; otherwise something else takes over the canvas next.
   */
  function stop(thenDenoise = false) {
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
    if (!job) return;
    const kind = job.kind;
    job = null;
    shows = 'stopped';
    const size = renderer.getDrawingBufferSize(new THREE.Vector2());
    const p = /** @type {Progress} */ ({ state: 'stopped', kind, samples: Math.floor(tracer.samples), width: size.x, height: size.y });
    report(p);
    lastPicture = tracer.samples >= 1 ? p : null;
    if (thenDenoise && settings.denoise && tracer.samples >= 1) denoise(p);
  }

  // ---- The denoiser: Intel Open Image Denoise (oidn-web, on WebGPU) with the albedo and normals
  // as guides; the path tracer's edge-aware smoothing where there's no WebGPU.

  /**
   * The denoised picture at the denoise strength: a blend, pixel by pixel, of the picture as
   * rendered and as denoised (1: all denoised). Less keeps some of the rendered grain and detail.
   */
  function blend() {
    if (!clean?.texture || !clean.raw || !clean.den) return;
    const data = /** @type {Float32Array} */ (clean.texture.image.data);
    blendDenoised(data, clean.raw, clean.den, clean.taken ?? null, settings.denoiseStrength, settings.denoiseDetail);
    clean.texture.needsUpdate = true;
  }

  function clearDenoised() {
    denoiseRun++;
    denoiseAbort?.();
    denoiseAbort = null;
    clean?.texture?.dispose();
    clean = null;
  }

  /** The network the settings ask for, loaded once. */
  async function loadUnet() {
    const file = UNETS[settings.denoiseModel] || UNETS.detailed;
    if (!unets.has(file)) {
      unets.set(
        file,
        import('oidn-web').then(({ initUNetFromURL }) =>
          // The weights for clean guides (calb, cnrm): the albedo and normals here are drawn, not
          // path traced, so free of noise; told so, the network keeps the detail they show (the
          // beads' round faces: the layer lines) instead of smoothing it away with the noise.
          initUNetFromURL(new URL(`../vendor/oidn-web/${file}`, import.meta.url).href, undefined, { aux: true, hdr: true }),
        ),
      );
    }
    try {
      return await unets.get(file);
    } catch (err) {
      unets.delete(file);
      throw err;
    }
  }

  /** One run of the network over the image, as a promise. */
  function execute(net, input) {
    return new Promise((resolve, reject) => {
      denoiseAbort = net.tileExecute({ ...input, done: resolve, error: reject });
    });
  }

  /**
   * The denoiser's guides, drawn from the camera at the picture's size: each surface's albedo
   * (its colour; the glowing bulb white) and normal (as colour, 0.5 + n / 2), the background
   * empty. The faces inside the walls are left out (they're not seen).
   */
  function renderAux(w, h) {
    const rt = new THREE.WebGLRenderTarget(w, h, { samples: 4 });
    /** @type {Map<THREE.Mesh, THREE.Material>} */
    const saved = new Map();
    scene.traverse((o) => {
      if (/** @type {any} */ (o).isMesh) saved.set(/** @type {THREE.Mesh} */ (o), /** @type {THREE.Material} */ (/** @type {THREE.Mesh} */ (o).material));
    });
    const inside = [...saved.keys()].filter((o) => o.name === 'part-inside');
    for (const o of inside) o.visible = false;
    const albedo = (m) => new THREE.MeshBasicMaterial({ color: m.emissiveIntensity > 0 ? 0xffffff : m.color ?? 0xffffff, side: m.side });
    const normal = (m) =>
      new THREE.ShaderMaterial({
        side: m.side,
        vertexShader: 'varying vec3 vN; void main() { vN = mat3(modelMatrix) * normal; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }',
        fragmentShader: 'varying vec3 vN; void main() { vec3 n = normalize(vN); if (!gl_FrontFacing) n = -n; gl_FragColor = vec4(n * 0.5 + 0.5, 1.0); }',
      });
    const out = {};
    const clear = renderer.getClearAlpha();
    for (const [key, make] of /** @type {const} */ ([['albedo', albedo], ['normal', normal]])) {
      const made = new Map();
      for (const [o, m] of saved) {
        if (!made.has(m)) made.set(m, make(m));
        o.material = made.get(m);
      }
      renderer.setRenderTarget(rt);
      renderer.setClearColor(0x000000, 0);
      renderer.clear();
      renderer.render(scene, camera);
      const px = new Uint8Array(w * h * 4);
      renderer.readRenderTargetPixels(rt, 0, 0, w, h, px);
      out[key] = new ImageData(new Uint8ClampedArray(px.buffer), w, h);
      for (const m of made.values()) m.dispose();
    }
    for (const [o, m] of saved) o.material = m;
    for (const o of inside) o.visible = true;
    renderer.setRenderTarget(null);
    renderer.setClearColor(0x000000, clear);
    rt.dispose();
    return /** @type {{ albedo: ImageData, normal: ImageData }} */ (out);
  }

  /** The edge-aware smoothing (no WebGPU): drawn straight from the path tracer's picture. */
  let smoother = /** @type {FullScreenQuad | null} */ (null);

  /** Draws the picture: denoised when there is one and it's showing, else as rendered. */
  function present() {
    const quad = /** @type {any} */ (tracer)._quad;
    renderer.setRenderTarget(null);
    if (clean?.on && clean.by === 'smooth') {
      smoother ||= new FullScreenQuad(
        Object.assign(new DenoiseMaterial({ sigma: 5, threshold: 0.03, kSigma: 1 }), { premultipliedAlpha: renderer.getContextAttributes().premultipliedAlpha }),
      );
      /** @type {any} */ (smoother.material).map = tracer.target.texture;
      /** @type {any} */ (smoother.material).sigma = 0.5 + 4.5 * Math.min(1, Math.max(0, settings.denoiseStrength));
      smoother.render(renderer);
      return;
    }
    if (!quad) return;
    // The path tracer's own display pass (its tone mapping, exposure and alpha), on either picture.
    quad.material.map = clean?.on && clean.texture ? clean.texture : tracer.target.texture;
    quad.material.opacity = 1;
    quad.material.blending = THREE.NoBlending;
    quad.render(renderer);
  }

  /** Denoises the picture as it is (a finished or stopped render) and shows the result. */
  async function denoise(p) {
    clearDenoised();
    const run = denoiseRun;
    const target = tracer.target;
    const [w, h] = [target.width, target.height];
    report({ ...p, state: 'denoising' });
    task = 'denoising';
    const moot = () => run !== denoiseRun;
    try {
      let by = /** @type {'oidn' | 'smooth'} */ ('smooth');
      /** @type {THREE.DataTexture | null} */
      let texture = null;
      let reason = '';
      /** @type {Float32Array | undefined} */
      let denRaw;
      /** @type {Float32Array | undefined} */
      let denOut;
      /** @type {Float32Array | undefined} */
      let taken;
      if ('gpu' in navigator) {
        try {
          const net = await loadUnet();
          if (moot()) return;
          const color = new Float32Array(w * h * 4);
          renderer.readRenderTargetPixels(target, 0, 0, w, h, color);
          // Open Image Denoise takes 0 to 65504: the path tracer now and then leaves a negative
          // value (a silk's metallic coat), which the display clips but the denoiser's transfer
          // curve turns into NaN, and a NaN spreads to the whole picture (black).
          for (let i = 0; i < color.length; i++) {
            const v = color[i];
            color[i] = v > 0 ? Math.min(v, 65504) : 0;
          }
          // Fireflies brought down first (denoise-prep.js): spread by the network, they made
          // blotches and a brighter picture.
          tameFireflies(color, w, h);
          const { albedo, normal } = renderAux(w, h);
          const input = { color: { data: color, width: w, height: h }, albedo, normal };
          const rgb = /** @type {{ data: Float32Array }} */ (await execute(net, input));
          if (moot()) return;
          const out = new Float32Array(rgb.data);
          let bad = 0;
          for (let i = 0; i < out.length; i++) if (!Number.isFinite(out[i])) bad++;
          if (bad) throw new Error(`the denoiser gave ${bad} invalid values`);
          // The alpha as rendered (oidn-web hands back alpha 1 everywhere: a black background).
          // Where it's partly see-through over a good part of the picture (a clear shade over the
          // transparent background), it's denoised too: as a grey picture of its own, through
          // the same network, the guides the same.
          let partial = 0;
          for (let i = 3; i < color.length; i += 4) if (color[i] > 0.02 && color[i] < 0.98) partial++;
          if (partial > (w * h) / 50) {
            const grey = new Float32Array(color.length);
            for (let i = 0; i < color.length; i += 4) {
              grey[i] = grey[i + 1] = grey[i + 2] = color[i + 3];
              grey[i + 3] = 1;
            }
            const a = /** @type {{ data: Float32Array }} */ (await execute(net, { ...input, color: { data: grey, width: w, height: h } }));
            if (moot()) return;
            for (let i = 3; i < out.length; i += 4) out[i] = color[i] <= 0 || color[i] >= 1 ? color[i] : Math.min(1, Math.max(0, a.data[i - 3]));
          } else {
            for (let i = 3; i < out.length; i += 4) out[i] = color[i];
          }
          denRaw = color;
          denOut = out;
          taken = takenDetail(color, out, w, h);
          texture = new THREE.DataTexture(new Float32Array(out.length), w, h, THREE.RGBAFormat, THREE.FloatType);
          // Read texel by texel (and float textures can't always be filtered: one that can't
          // reads as black).
          texture.minFilter = THREE.NearestFilter;
          texture.magFilter = THREE.NearestFilter;
          texture.needsUpdate = true;
          by = 'oidn';
        } catch (err) {
          reason = String(/** @type {any} */ (err)?.message || err);
        }
      } else {
        reason = 'no WebGPU';
      }
      if (moot()) {
        texture?.dispose();
        return;
      }
      clean = { texture, by, on: true, last: p, raw: denRaw, den: denOut, taken };
      blend();
      present();
      report({ ...p, denoised: by, showing: true, ...(reason ? { denoiseError: reason } : {}) });
    } catch (err) {
      if (!moot()) report({ ...p, denoiseError: String(/** @type {any} */ (err)?.message || err) });
    } finally {
      if (!moot()) denoiseAbort = null;
    }
  }

  /** Settings applied: the room, the camera, the light; the draft shows them. */
  function applySettings(next) {
    const before = settings;
    settings = next;
    // Only the denoiser's settings changed: the picture stays, re-blended at the new strength.
    const changed = Object.keys(settings).filter((k) => JSON.stringify(before[k]) !== JSON.stringify(settings[k]));
    if (changed.every((k) => DENOISE_KEYS.includes(k))) {
      // Another network: the picture denoised again with it; else re-blended.
      if (changed.includes('denoiseModel') && clean && !job && lastPicture) denoise(lastPicture);
      else if (clean) {
        blend();
        present();
      }
      return;
    }
    stop();
    const roomChanged = ['wall', 'floor', 'ceiling'].some((k) => JSON.stringify(before[k]) !== JSON.stringify(settings[k]));
    if (snap && roomChanged) buildRoom();
    if (before.camera.lens !== settings.camera.lens && snap) {
      // A new lens keeps what's framed: the camera moves to keep the same width in view.
      const old = Math.tan(THREE.MathUtils.degToRad(lensFov(before.camera.lens) / 2));
      const now = Math.tan(THREE.MathUtils.degToRad(lensFov(settings.camera.lens) / 2));
      const off = camera.position.clone().sub(controls.target).multiplyScalar(old / now);
      camera.position.copy(controls.target).add(off);
      camera.fov = lensFov(settings.camera.lens);
      clip();
      controls.update();
      saveCamera();
    } else if (snap && (!settings.camera.position || before.camera.view !== settings.camera.view)) {
      placeCamera();
    }
    applyLight();
    draft();
  }

  /** What waited for the next picture, applied (before a Preview or Render, or a new view). */
  function applyPending() {
    const [next, looks] = [pendingSettings, pendingLooks];
    pendingSettings = null;
    pendingLooks = null;
    if (looks) {
      for (const [part, pair] of partMaterials) setLook(pair, looks[part] || { color: '#f2f2f2', opacity: 1 });
      if (snap) snap.looks = looks;
    }
    if (next) applySettings(next);
  }

  return {
    /**
     * Prepares a snapshot for rendering: every toolpath swept into solid beads, the fittings.
     * Slow; `onStep` says what it's doing. The camera stays where it was left (fitted the first
     * time).
     * @param {Snapshot} next
     * @param {(what: string) => void} [onStep]
     */
    async prepare(next, onStep = () => {}) {
      stop();
      clearDenoised();
      lastPicture = null;
      // What waited applies (the looks come with the snapshot).
      if (pendingSettings) settings = pendingSettings;
      pendingSettings = null;
      pendingLooks = null;
      shows = 'empty';
      snap = null;
      if (fixture) {
        fixture.parent?.remove(fixture);
        fixture = null;
      }
      scene.remove(root);
      disposeGroup(root);
      for (const pair of partMaterials.values()) for (const m of pair) m.dispose();
      partMaterials.clear();
      root = new THREE.Group();
      // Recipe (z up, mm) to scene (y up, m); hung the other way: turned over first.
      root.rotation.x = -Math.PI / 2;
      root.scale.setScalar(MM);
      hung = new THREE.Group();
      if (next.fixture?.flipped) hung.rotation.x = Math.PI;
      root.add(hung);
      scene.add(root);
      const nextFrame = () => new Promise((resolve) => requestAnimationFrame(() => resolve(undefined)));

      let triangles = 0;
      for (let k = 0; k < next.paths.length; k++) {
        onStep(`Beads ${k + 1}/${next.paths.length}`);
        task = 'preparing';
        await nextFrame();
        const p = next.paths[k];
        const n = p.points.length / 3;
        const white = new Float32Array(3 * n).fill(1);
        // Where the turns touch: the flat faces where one sits on the next go inside the solid,
        // only as far as the next covers them (on a sloping wall each turn sits partly beside
        // the last: the rest of the face is outside, a face like the beads' round ones).
        const { above, below, aboveAt, belowAt } = touching(p.points, p.widths, p.heights, p.ups);
        const { geometry, kinds, at } = beadGeometry(p.points, p.widths, p.heights, white, p.ups, { kinds: true, cover: { aboveAt, belowAt } });
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
        if (!partMaterials.has(p.part)) {
          const pair = newPair();
          setLook(pair, next.looks[p.part] || { color: '#f2f2f2', opacity: 1 });
          partMaterials.set(p.part, pair);
        }
        // The covered faces go inside the solid (a mesh of their own).
        const outerIx = [];
        const innerIx = [];
        for (let t = 0; t < ix.length / 3; t++) {
          const kind = kinds ? kinds[t] : 0;
          const i = at ? at[t] : 0;
          const inside = (kind === 1 && above[i]) || (kind === 2 && below[i]);
          (inside ? innerIx : outerIx).push(ix[3 * t], ix[3 * t + 1], ix[3 * t + 2]);
        }
        // Two meshes, one material each: the beads' outside, and the faces inside the wall with
        // vertices of their own. (The path tracer keeps a material per vertex, and numbers the
        // materials of its merged scene by mesh: a mesh with two materials, or a vertex shared
        // by both, gives the wrong material to whatever comes after.)
        const pos = /** @type {Float32Array} */ (geometry.getAttribute('position').array);
        const verts = pos.length / 3;
        geometry.setIndex(new THREE.BufferAttribute(verts > 65535 ? new Uint32Array(outerIx) : new Uint16Array(outerIx), 1));
        geometry.computeVertexNormals();
        const [outerMat, innerMat] = /** @type {THREE.MeshPhysicalMaterial[]} */ (partMaterials.get(p.part));
        const outer = new THREE.Mesh(geometry, outerMat);
        outer.name = 'part';
        hung.add(outer);
        if (innerIx.length) {
          const inside = new THREE.BufferGeometry();
          const flat = new Float32Array(innerIx.length * 3);
          innerIx.forEach((v, k) => flat.set(pos.subarray(3 * v, 3 * v + 3), 3 * k));
          inside.setAttribute('position', new THREE.BufferAttribute(flat, 3));
          const seq = innerIx.length > 65535 ? new Uint32Array(innerIx.length) : new Uint16Array(innerIx.length);
          for (let k = 0; k < seq.length; k++) seq[k] = k;
          inside.setIndex(new THREE.BufferAttribute(seq, 1));
          inside.computeVertexNormals();
          const mesh = new THREE.Mesh(inside, innerMat);
          mesh.name = 'part-inside';
          hung.add(mesh);
        }
      }
      snap = next;
      buildRoom();
      sizeFor(0);
      placeCamera();
      draft();
      return { triangles };
    },

    /** Whether there's a prepared scene. */
    get prepared() {
      return Boolean(snap);
    },

    /** Whether a render is running. */
    get running() {
      return Boolean(job);
    },

    /**
     * New settings: the render stops (nothing renders until Preview or Render), the draft shows
     * the change. A new view or lens refits; surfaces and the ceiling rebuild the room.
     * @param {ReturnType<typeof cleanRender>} next
     */
    setSettings(next) {
      const incoming = cleanRender(next);
      if (held()) {
        // The denoiser's settings apply to the picture now; the rest wait for the next one.
        const base = pendingSettings || settings;
        const changed = Object.keys(incoming).filter((k) => JSON.stringify(base[k]) !== JSON.stringify(incoming[k]));
        const now = changed.filter((k) => DENOISE_KEYS.includes(k));
        if (now.length) {
          const before = settings;
          settings = { ...settings, ...Object.fromEntries(now.map((k) => [k, incoming[k]])) };
          if (before.denoiseModel !== settings.denoiseModel && clean && !job && lastPicture) denoise(lastPicture);
          else if (clean) {
            blend();
            present();
          }
        }
        if (changed.some((k) => !DENOISE_KEYS.includes(k)) || pendingSettings) pendingSettings = incoming;
        report(lastReport);
        return;
      }
      applySettings(incoming);
    },

    /**
     * The parts' looks (custom colour, how solid, material), by part. Stops the render.
     * @param {Look[]} looks
     */
    setLooks(looks) {
      if (held()) {
        pendingLooks = looks;
        report(lastReport);
        return;
      }
      for (const [part, pair] of partMaterials) setLook(pair, looks[part] || { color: '#f2f2f2', opacity: 1 });
      if (snap) snap.looks = looks;
      draft();
    },

    /**
     * Throws the picture away (a render under way stops): back to the draft, with whatever
     * settings and looks waited, the camera free again.
     */
    discard() {
      stop();
      clearDenoised();
      lastPicture = null;
      shows = 'draft';
      applyPending();
      sizeFor(0);
      draft();
    },

    /** Whether there's a picture held (under way, done or stopped). */
    get held() {
      return held();
    },

    /** Whether settings or looks wait for the next picture. */
    get pending() {
      return Boolean(pendingSettings || pendingLooks);
    },

    /** Frames the lamp again from the view set (forgets where the camera was left). */
    fit() {
      stop();
      clearDenoised();
      shows = 'draft';
      applyPending();
      fit(settings.camera.view);
      saveCamera();
      draft();
    },

    /**
     * Renders: a preview (small, few samples) or the picture (its size and samples), with the
     * bounces as set, until done or stopped.
     * @param {'preview' | 'final'} kind
     */
    start(kind) {
      if (!snap || lost) return;
      stop();
      clearDenoised();
      // The picture before is let go; what waited for this one applies.
      shows = 'draft';
      applyPending();
      const q = kind === 'final' ? settings.final : settings.preview;
      sizeFor(q.size);
      applyLight();
      for (const [part, pair] of partMaterials) setLook(pair, snap.looks[part] || { color: '#f2f2f2', opacity: 1 });
      if (sceneDirty) {
        // The path tracer merges the scene into one geometry it keeps, adding a group per mesh
        // each time without clearing the last time's: cleared here, or a second scene keeps the
        // first one's materials on its meshes.
        /** @type {any} */ (tracer)._generator?.geometry?.clearGroups?.();
        task = 'loading the scene onto the GPU';
        tracer.setScene(scene, camera);
        sceneDirty = false;
      } else {
        tracer.setCamera(camera);
      }
      tracer.updateMaterials();
      tracer.updateLights();
      tracer.updateEnvironment();
      // Tiles: each GPU dispatch a bounded amount of work.
      const px = renderer.getDrawingBufferSize(new THREE.Vector2());
      // A see-through part's paths go on through it (up to the transmissive bounces): more work.
      const seeThrough = [...partMaterials.values()].some(([outer]) => outer.transmission > 0);
      const work = px.x * px.y * (settings.bounces + 2 + (seeThrough ? tracer.transmissiveBounces / 2 : 0));
      const budget = phone ? DISPATCH.phone : APPLE ? DISPATCH.apple / care() : DISPATCH.other;
      const n = Math.min(APPLE ? MAX_TILES_APPLE : MAX_TILES, Math.max(1, Math.ceil(Math.sqrt(work / budget))));
      tracer.tiles.set(n, n);
      clearFence();
      tracer.reset();
      job = { kind, target: q.samples, t0: performance.now() };
      report({ state: tracer.isCompiling ? 'compiling' : 'rendering', kind, samples: 0, target: q.samples });
      frame = requestAnimationFrame(tick);
    },

    /** Stops rendering; the picture so far stays (denoised, when that's on) and can be saved. */
    stop() {
      stop(true);
    },

    /** Denoises the picture showing (a finished or stopped render), at the button's press. */
    denoise() {
      if (job || !lastPicture || !snap) return;
      denoise(lastPicture);
    },

    /** Whether there's a picture to denoise (finished or stopped, nothing running). */
    get canDenoise() {
      return !job && Boolean(lastPicture) && (shows === 'done' || shows === 'stopped');
    },

    /**
     * Shows the denoised picture (on) or the one as rendered (off), when there's a denoised one.
     * @param {boolean} on
     */
    showDenoised(on) {
      if (!clean) return;
      clean.on = on;
      present();
      report({ ...clean.last, denoised: clean.by, showing: on });
    },

    /** After the view was hidden: the draft again if nothing rendered is showing. */
    resume() {
      if (!job && shows !== 'done' && shows !== 'stopped') draft();
    },

    /** Stops sampling (the view hidden). */
    pause() {
      stop();
    },

    /** Whether the canvas holds a rendered picture (done, stopped, or under way). */
    get rendered() {
      return shows === 'done' || shows === 'stopped' || shows === 'rendering';
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
      stop();
      snap = null;
      sizes.disconnect();
      controls.dispose();
      scene.remove(root);
      disposeGroup(root);
      disposeGroup(room);
      for (const pair of partMaterials.values()) for (const m of pair) m.dispose();
      for (const m of Object.values(fittings)) m.dispose();
      surfaceMaterial.dispose();
      clearDenoised();
      unet?.dispose?.();
      unet = null;
      smoother?.dispose();
      draftMaterial.dispose();
      sky.dispose();
      // (A lost context has nothing to free; three.js mostly ignores the calls, but not always.)
      try {
        tracer.dispose();
        renderer.dispose();
        if (!lost) renderer.forceContextLoss();
      } catch {
        // the context is gone already
      }
    },
  };
}
