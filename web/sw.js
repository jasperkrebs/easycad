/* Offline cache for the app shell, same approach as EasyGCode's sw.js.
 *
 * Network-first for everything on this site, revalidating past the HTTP cache, so a plain
 * refresh always gets the latest deploy when online. The cache is only the offline fallback.
 * One fixed cache name: nothing to bump per release. Edit ASSETS only when adding or removing
 * a shipped file.
 */
const CACHE = 'easycad';
const ASSETS = [
  './',
  './index.html',
  './styles.css',
  './manifest.webmanifest',
  './js/app.js',
  './js/dom.js',
  './js/engine.js',
  './js/engine.worker.js',
  './js/print.js',
  './js/recipe.js',
  './js/settings.js',
  './js/viewport.js',
  './templates/twisted-vase.easycad',
  './templates/ribbed-cylinder.easycad',
  './templates/star-bowl.easycad',
  './pkg/easycad_wasm.js',
  './pkg/easycad_wasm_bg.wasm',
  './vendor/three/three.module.js',
  './vendor/three/three.core.js',
  './vendor/three/addons/controls/OrbitControls.js',
  './vendor/three/addons/lines/Line2.js',
  './vendor/three/addons/lines/LineGeometry.js',
  './vendor/three/addons/lines/LineMaterial.js',
  './vendor/three/addons/lines/LineSegments2.js',
  './vendor/three/addons/lines/LineSegmentsGeometry.js',
  './icons/icon-192.png',
  './icons/icon-512.png',
];

self.addEventListener('install', (e) => {
  e.waitUntil(
    caches
      .open(CACHE)
      .then((c) => c.addAll(ASSETS))
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener('activate', (e) => {
  e.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k))))
      .then(() => self.clients.claim()),
  );
});

self.addEventListener('fetch', (e) => {
  if (e.request.method !== 'GET') return;
  const url = new URL(e.request.url);
  if (url.origin !== self.location.origin) return;
  const isNav = e.request.mode === 'navigate';
  e.respondWith(
    fetch(e.request, { cache: 'no-cache' })
      .then((resp) => {
        // Never let a 404/500 from a half-finished deploy overwrite a good cached copy.
        if (resp.ok) {
          const copy = resp.clone();
          caches.open(CACHE).then((c) => c.put(e.request, copy)).catch(() => {});
        }
        return resp;
      })
      .catch(() =>
        caches.match(e.request).then((hit) => hit || (isNav ? caches.match('./index.html') : undefined)),
      ),
  );
});
