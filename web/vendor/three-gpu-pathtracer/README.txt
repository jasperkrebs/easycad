three-gpu-pathtracer 0.0.27 (npm "three-gpu-pathtracer", build/index.module.js)
https://github.com/gkjohnson/three-gpu-pathtracer — MIT, see LICENSE
0.0.27 is the newest release that works with three.js 0.186 (0.0.28 needs 0.187).
Loaded only by the Render view (web/js/render.js).

One line changed (EasyCAD): in the path tracing loop, a surface that casts no shadow is skipped
by a "shadow ray" only when that ray is not still the camera's view through see-through surfaces:
  if ( ! material.castShadow && state.isShadowRay && ! state.transmissiveRay ) {
(was: if ( ! material.castShadow && state.isShadowRay ) {). The bulb casts no shadow (its light
inside must get out); unchanged, a ray seen through a rough see-through shade passed through the
bulb to the background, and the bulb was missing from the picture. Re-apply when updating.
