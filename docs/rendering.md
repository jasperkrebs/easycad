# The Render view: the lamp's light, path traced

How a lampshade looks with its bulb on (and off) depends on how the light bounces off it,
passes through it and bends through its beads. The **Render** view (next to Shape, Toolpath,
Both, Plan) works that out with a path tracer and saves a PNG with a transparent background,
to place on top of an environment afterwards.

## The fittings: socket, bulb, cable

Every recipe with a `socket` setting (Lampshade, Lamp, Thread pendant) shows a standard
lampholder where the shade screws on, a frosted bulb in it and a cable going straight up, in
the 3D view, the Plan and the render (`web/js/fixture.js`, its meshes `fixture-mesh.js`).

| Socket | Holder (body Ø × height) | Bulb | Glass Ø | Glass below the cap |
|---|---|---|---|---|
| E27 | 40 × 55 mm | A60 (pear) | 60 mm | 83 mm (about 108 mm with the cap) |
| E14 | 28 × 45 mm | P45 (round) | 45 mm | 58 mm (about 80 mm with the cap) |
| custom | the recipe's thread (`thread_d` or `socket_d`) × `socket_h` (else 55) | A60, P45 under 34 mm | | |

The holder sizes are the ones the recipes already use (the shade's thread on the body, the
Thread pendant's body heights); the bulbs are the IEC 60064 shapes, sizes as manufacturers
list them (worth checking against a datasheet before designing to the millimetre). The cable
is 6 mm across.

**Where it goes:** on the straight section that screws onto the socket: the part whose beads run
round the holder's body (within three quarters of a bead of it). The socket always points down:
its face (where the bulb comes out) is at that section's lower end as the lamp hangs, the body
rising from there to the cable.

**Which way it hangs:** the shades down past the bulb (a pendant, the default) or up away from it,
the **Hangs ↓ / ↑** button in the Render view's row. The project is turned over for it if need
be: the view setting is kept with the project's colours (in its file too); the recipe is
untouched. The 3D view and the Plan show the project as it prints (throat on the bed), the
fittings placed as the lamp hangs; the render shows it hanging.

## Two steps: prepare, then render

- **Prepare** takes a snapshot of the project as it is: each printed part's toolpaths swept into
  solid beads at their real line width and layer height (the 3D view's *Line width* beads,
  `beadGeometry`, so the layer lines are real round faces), the fittings, the custom colours and
  the hidden parts (left out, as the brim is). Then the BVH (the structure the rays search) is
  built. Slow, once: changing the project afterwards changes nothing in the render until it's
  prepared again (the Prepare button lights up when it's out of date).
- **Render** adds samples until the set number (64 to 4096 per pixel), the picture getting less
  grainy with each. Orbiting, the light and the settings only restart the sampling.

## The light

- **Light on:** the bulb is the main light: a point light at the glass's centre (its colour
  temperature, 2700 K by default, and its output in lumens, 806 for a 60 W bulb), the frosted
  glass glowing as bright as the light it lets out (and not shading the light inside it). A
  little light from round the lamp (**Room**, 6 % by default) keeps the dark side from going
  black.
- **Light off:** daylight from round the lamp only (a bright sky over a darker floor); the bulb is
  plain frosted glass.

The scene is in metres, so the light falls off with distance as it does in a room. AgX tone
mapping and the **Exp** slider (± stops) bring the bulb's glare and the shade's glow into one
picture.

## The materials: from the custom colours

Each printed part takes its **custom colour** and how **solid** it is (the Custom colour mode's
controls, `docs/architecture.md` §9):

- **Solid** (100 %): the light bounces off it, scattered (a white shade reflects the bulb's
  light round the inside and out of the opening).
- **See-through** (less): that much of the light passes into the plastic (transmission = 1 −
  solid), bends at each bead's round face, is tinted by the colour over a few millimetres, and
  comes out again: the layer lines show as bright and dark bands, as on a print held to the
  light.
- **Matte** sets how rough the printed surface is (glossy to frosted), for the reflections and
  for the light passing through.

The plastic's refractive index is 1.5 (PLA and PETG are 1.46 to 1.57).

## The picture

The background stays transparent (the checkerboard behind the view); the lamp's own pixels are
solid where it's opaque and partly transparent where the background shows through a clear
shade, so it composites over any environment. **Resolution** renders at ½, 1 or 2 × the
screen's pixels (at most 4096 across); **PNG** saves the picture as it is.

On a phone the work is cut into tiles so the page stays responsive; it's slow there (a lamp of
a million triangles is fine on a laptop's GPU), but preparing and setting up a render works
anywhere.

## How it's built

- **three-gpu-pathtracer** 0.0.27 (MIT, by Garrett Johnson; with **three-mesh-bvh** 0.9.18):
  an unbiased path tracer in a WebGL 2 shader, on the three.js scene the app already draws, with
  three's physical materials (transmission, roughness, attenuation, emission), point lights and
  environment light. 0.0.27 is the newest that works with our three.js 0.186 (0.0.28 needs
  0.187); vendored in `web/vendor/`, loaded only when the Render view first opens
  (`web/js/render.js`).
- The bead sweep winds its faces inward (the 3D view draws both sides); the render turns them
  outward, as the path tracer reads in and out of a solid from them.
- **One solid wall** (the "solidization"): beads of consecutive turns touch at their flat tops
  and bottoms. Left as surfaces, light crossing from one turn into the next would refract out of
  one bead and into the next, and be trapped in each turn by total internal reflection: a
  see-through shade came out dark. So the faces where a turn sits on the next (`web/js/wall.js`
  finds them: a bead one layer height up, within half a bead across) get a material with the
  same refractive index on both sides: light crosses them unbent, tinted as it goes. The wall's
  outside is then its beads' round faces, inside and out, where the light bends: the layer lines'
  lensing. The first turn's bottom, the last turn's top and any step where a turn sits out past
  half a bead stay real faces.
- The library's WebGL path tracer is marked deprecated in favour of its WebGPU one; the WebGPU
  one is the upgrade once phones run WebGPU everywhere.

Not yet (on the roadmap): a denoiser (fewer samples for a clean picture), your own environment
(an HDRI) for the light off, the light falling on a wall or ceiling as a layer of its own, and
building the BVH in a worker (preparing blocks the page while it runs).
