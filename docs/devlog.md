# Dev log

Newest first. Each entry: what landed, numbers, anything left open (which also goes on the
roadmap's "Open" list). Keep it short; it's how the next session (human or agent) picks up.
Headings end with the branch that wrote them (`general-manager` or `recipe-developer`,
[working-together.md](working-together.md)).

Archives: [October 2026](devlog-2026-10.md), [November 1–14](devlog-2026-11.md).

## 2026-11-23: Nutcracker: spokes as long as they can be, or a spiral (recipe-developer)

**Why:** the owner: more than 4 spokes ran into each other in the middle, fewer didn't meet; 1
should run across to the opposite wall, 2 meet tip to tip, 3 like the Mercedes star, more get
shorter. And, instead of spokes, the bottom's spiral again on top, the other way round, in to the
middle and back out between, into the wall, in one line.

- **Spoke length** (`spokes`, `inner` 0): c = w / sin(180°/n) from the axis (bands two lines wide,
  rounded ends, just touching); one spoke: across, a bead off the opposite wall. 4 moves from
  1.25w to 1.41w (its tips overlapped a little). Drawn from above for 1–12 and tested (1, 2, 3, 4, 6).
- **`spiral_tie(path, height, width, count, layer)`** (new step): rings a line apart, in against
  the wall's way, round in the middle, out between them, all ring changes in one window next to
  where it goes in, the last onto the wall carrying straight on. A first try (a steady spiral, 2
  lines a turn) left a crescent against the wall; rings fixed it. Unit- and template-tested.
- `Cap/Key: tied to the wall by` (spokes, spiral); `how many, on top of each other` for both.
- `spokes` takes `layers`/`layer` (one step for stacked crosses: five chained steps copied the
  path five times and ran out of points on a 300 mm cap with 1 mm lines); a count of 0 passes the
  path through. A sweep (thread 20–300 mm, line 1–12 mm, 1–12 spokes, 1 and 5 stacked, both ties):
  no errors; where there's no room, it says how many were left out.

Open: a test print of both.


## 2026-11-23: test 8 is every template's overhang default (general-manager)

**Why:** the owner's test 8 (strength 86 %, steep 0.85, pivot 73°; the Bambu with a 0.8 mm
nozzle, E14, 5 mm line, flow 15) printed very well, and the owner asked for it as the default in
every project with overhang compensation, and to call it "test 8" from now on. Done on the
owner's word in the recipe developer's files.

- Defaults `strength`/`pace` 100 → 86 %, `steep` 0 → 0.85, `pivot` 45 → 73° in the Lampshade,
  the Lamp, the Thread pendant, the Nutcracker, the vessel and the salt shaker.
- The golden tests compare with EasyGCode at EasyGCode's compensation (full strength, no steep
  curve): `as_easygcode` and the smoke test set those along with the rest of EasyGCode's setup;
  the fixtures are unchanged. Three recipe tests that measure the even, full compensation pin
  it (`strength: 100`, `steep: 0`).
- New projects from a template start at test 8; a saved `.easycad` file keeps the values written
  in it.

## 2026-11-23: Nutcracker: stacked crosses, spokes per part (recipe-developer)

**Why:** the owner: several crosses printed on top of each other, set for the cap (bottom) and
the key (top) each, and each part's own number of spokes.

- `Cap: spokes`, `Cap: crosses on top of each other`, `Key: spokes`, `Key: crosses on top of each
  other` (up to 5) replace `Spokes on the bottom`. Each further cross is a layer up, flat on the
  one below.
- **`spokes(…, angle)`** (engine): the first spoke where the path next passes that angle, so
  crosses at several heights stack spoke over spoke; the nutcracker's all start at the seam (+Y).
  Without it, as before. Unit-tested (two crosses at 45°) and in `nutcracker_tests.rs`.

Open: a test print.

## 2026-11-22: Patterns on the Lamp's and the Lampshade's sections (recipe-developer)

**Why:** the owner: tap sections in the Plan and put a pattern on them, one pattern running
through all the picked sections, fading in and out along the surface; first a sine wave in and
out, matching up into rails, or woven.

- **Pattern** group in the Lampshade and the Lamp's shade: a switch per section (start/throat,
  fillet, shade, end fillet, straight end/lip), waves a turn, rails or woven (half a wave more a
  turn), depth, fade length along the surface. Off as it ships: the G-code is unchanged (golden
  tests). On the thread (start, the Lamp's straight end) it warns the thread is gone.
- **The Plan:** a mark with on/off settings gets its own bar (its name leads to the settings,
  its switches as chips, done); `@mark` keeps the section switches off the shade's own bar. A
  **Pattern** mark beside the patterned sections leads to the pattern's settings.
- **`wave`** (engine, with the owner's leave): an amplitude of 0 leaves the path as it is, and
  only the moves the wave reaches are split, by how far round each turns (the shade's spiral has
  no fixed count a turn, so every move was split 12 × count times: the Lamp's sphere ran out of
  points). Unit-tested.
- Tests: `patterns_tests.rs` (5), the `wave` test, the smoke test's Plan section.

Open: a test print; more patterns. See [patterns.md](patterns.md). Noticed, not from this change:
`scripts/request-test.cjs` stops at its line 144 (no "Send to be built" button), the same without it.

## 2026-11-22: Nutcracker: a cross on each bottom, overhang compensation, exact threads (recipe-developer)

**Why:** the owner: overhang compensation for the fillets, and strong bottoms: after the bottom's
spiral and a layer up the fillet, the line should run in to the middle and back, 4 times, a cross.

- **`spokes(path, height, count, width, inner)`** (new step, `toolpath_ops.rs`; general manager's
  area, the owner's leave): from where a path round the Z axis first reaches `height`, `count`
  spokes spread over the turn after, each in to near the axis and back out (legs a line apart, a
  half circle round), flat at `height`, the bit of wall between the legs left out. Unit-tested on a
  helix. In the nutcracker at two layers up, 4 by default ("Spokes on the bottom", 0: none).
- **Overhangs** group (as the Lampshade's): compensation by layer height, a field of z from the
  fillets' quarter circles and the flattening's slope; replaces "Closer layers on overhangs".
- **Exact threads:** `contour` with a varying layer fitted its steps to the whole height, so the
  threads came out 2.474 and 2.478 mm a turn (cap, key); each part is now sliced in pieces (fillet
  and flattening compensated, threaded tube and handle at the pitch): exactly 2.5 (tested).
- From the sweep: a handle can't be narrower than it's thick, and a flattening is lengthened to
  stay under about 70° (shorter, its layers couldn't stack); both tested.

Open: a test print. Noticed, not from this change: a long fuzz round (FUZZ_CASES=800) finds a
damaged copy of `fixtures/recipes/sphere-robot.easycad` taking 29 s (the same on `main`).

## 2026-11-22 (night): denoise on request, layer lines kept; the socket's shadow (claude/lampshade-light-render-wcvx6x)

- **Denoise is a button**, off until pressed (beside PNG, on a finished or stopped picture);
  **Denoise by itself** in Quality runs it on every finished one; **Denoise strength** blends the
  picture as rendered back in, at once, without rendering again.
- **Layer lines kept:** the denoiser now uses Intel's network for *clean* guides
  (`rt_hdr_calb_cnrm`): the albedo and normals here are drawn, not path traced, and told so, the
  network keeps what the normals show (the beads' round faces). The network for noisy guides had
  smoothed the lines away with the grain. Measured on a frosted lamp at 640 px, 24 samples: the
  brightness changes about 4× (3.8–4.5) more from row to row than from column to column once denoised (the
  lines), 1.1 through the grain.
- **The socket's shadow:** checked with a render: hung up (a bowl over the socket, the bulb under
  it), no room light, the inside of the bowl stays dark: the socket blocks the bulb. The render
  showed faint rings of light inside the bowl, though: the faces between turns were see-through
  in a solid part too, and light slipped through the seams where one turn's bead meets the next.
  In a solid part they're solid now.
- `render-test`: denoising pressed (not by itself), the socket's shadow, the layer lines.

## 2026-11-22 (evening): the render's denoiser; the room as a cutaway (claude/lampshade-light-render-wcvx6x)

- **Denoiser:** Intel Open Image Denoise (oidn-web 0.5.0 on WebGPU, the `rt_hdr_alb_nrm` weights
  from `oidn-weights` 2.3.0-1) runs when a preview or render is done or stopped: the float picture
  with albedo and normal guides drawn from the camera, the alpha too where a shade is see-through.
  On the test scenes at 16 samples, the noise (a pixel's mean difference from its neighbours)
  drops from 14–18 to under 1. **Denoised** beside PNG compares it with the picture as rendered;
  **Denoise** in Quality turns it off. Without WebGPU: the path tracer's edge-aware smoothing.
- **Bug: the lamp hidden with wall, floor and ceiling on:** the surfaces were solid from both
  sides, so a camera above the ceiling (or below the floor, behind the wall) saw only them, while
  the draft (front faces only) showed the lamp. Now each is solid from the room's side only: a
  cutaway. `render-test` has the scene (all three on, from above) and checks the lamp shows.
- `render-test` runs WebGPU (software) too, and checks every scene is denoised and less noisy.
- Found on the way: the path tracer now and then leaves a negative value (a silk's metallic
  coat); the display clips it, but the denoiser's transfer curve made it NaN and the whole
  picture black. The input is clamped to 0–65504 (the denoiser's range), and an output with
  invalid values falls back to the smoothing, saying why. `render-test` checks the denoised
  picture's brightness too (a black one has no noise at all).
- oidn-web hands back alpha 1 everywhere (a black background), its alpha pass
  too; the render keeps its own alpha and, over a see-through shade, denoises it as a grey picture
  through the same network. The denoised picture is a float texture read with nearest filtering
  (a float texture some GPUs can't filter reads as black).
- `scripts/smoke-test.cjs` expects the Lampshade's new **Pattern** group (from the patterns change
  on `main`, whose smoke test still listed the old groups).

**Open:** at 16–32 samples the denoiser softens the layer lines of see-through shades.

## 2026-11-22 (later): the render's settings in a side tab, typed or slid (claude/lampshade-light-render-wcvx6x)

The Render view's settings moved from the row under the view to a **Render** tab in the side
panel (shown while the view is): fold-outs (Light, Camera, Room, a Material per part, Quality)
built from the settings window's own rows (`settingRow`, `foldout`, now exported from
`settings.js`), so every value has a number field beside its slider and units. Under the view:
only Prepare, Preview, Render, Stop and PNG.

## 2026-11-22: the render: consistent, rendered on demand, plastics, camera and room (claude/lampshade-light-render-wcvx6x)

**Why:** the owner found the render inconsistent (the same scene looking like another material
after a reload), live rendering ran the Mac's GPU out of memory and froze the browser, and asked
for plastics, per-part materials, a camera and a room ([rendering.md](rendering.md)).

**What landed:**
- **The inconsistency, found:** three-gpu-pathtracer merges the scene into one geometry and
  numbers its materials by *mesh* (a group per mesh, into the flat list of all meshes'
  materials), keeping a material per *vertex*, and adds those groups to the geometry it keeps
  without clearing the last scene's. Each part was one mesh with two materials (its beads; the
  hidden faces between turns), so every mesh after it took its neighbour's material (the socket
  the hidden faces', the bulb the cap's, a wall the bulb's glow), and a second Prepare kept the
  first scene's assignments. Now one material per mesh (each part two meshes, the hidden faces
  with vertices of their own), and the merged geometry's groups are cleared before each scene.
  And the hidden faces were a material of index 1 between solids: NaNs in the transmission,
  blackening half the pixels behind a see-through shade (measured on the frosted-PETG scene:
  42–50 % black pixels; as a thin film, 0.3 %). Also gone: the raster stand-in while shaders
  compile or the camera moves (it drew transmission another way); the noise is stable; the
  camera is kept with the project instead of copied from the 3D view at Prepare.
- **Rendered on demand:** Prepare, then set up (a plain grey draft frames the picture), then
  **Preview** (640 px, 32 samples) or **Render** (1920 px, 512 samples), **Stop**; the same
  bounces for both (12). Sizes are the picture's pixels whatever the screen; the GPU's work is cut
  into tiles by pixels × bounces; the high-performance GPU is asked for. Status: compiling,
  samples, pixels, time.
- **Plastics** (`web/js/materials.js`): PLA, PLA matte, PLA silk, PETG, PETG frosted, PC clear,
  PP, ABS/ASA, TPU, wood fill, stone fill; per part with its colour and how solid, and its own
  matte, coat, sheen, silk, depth and IOR (↺ back to the plastic's). Kept with the custom colours
  (in the file too); no Prepare needed to change them.
- **Camera:** views (Front, ¾, Side, Below, Above) with Fit, lenses 24–135 mm (a new lens keeps
  the framing), the orbit kept.
- **Room:** wall, floor and ceiling at set distances (the cable ends at the ceiling), lit by the
  lamp and in the PNG; one colour.
- The bulb is opal glass: solid in the PNG (it had gone black there: see-through glass gave it no
  alpha).
- `scripts/render-test.cjs` renders five scenes (Lampshade white PLA; frosted PETG with wall and
  floor; Lamp in amber PC from below; silk gold with the light off; Thread pendant with ceiling and
  wall) and checks Stop; `scripts/fixture.test.mjs` the plastics and settings.

**Open:** a denoiser (16–32 sample previews are grainy), the room's light as a layer over a
transparent background, the picture's own shape, your own environment (HDRI).

## 2026-11-21: the 3D view comes back after the phone takes its GPU away (general-manager)

**Why:** on the owner's phone the lamps no longer showed in the 3D view (shape, toolpath, both)
while the Plan, drawn as SVG, showed everything. Not reproducible in desktop Chromium or WebKit
with the templates; the failure that matches is a lost WebGL context that the browser never gives
back (Safari on a phone, short of GPU memory: since 21 Nov the Render view's path tracer holds a
second, large context even when the view is closed). Forced in Chromium, the live site stays
blank exactly so.

- `viewport.js`: on `webglcontextlost`, waits about a second for the context to come back
  (three.js picks that up); when it doesn't (and the page is visible), a fresh canvas in the
  same place with a new renderer; the scene uploads again. Three in a minute: "The 3D view
  stopped: … Reload the page." `viewport.listen` keeps the app's double-tap listeners on
  whichever canvas is current.
- `app.js` / `render.js`: before the rebuild the Render view's path tracer is freed
  (`dispose()`: the BVH, the beads, the targets, its context); while the Render view is open, it
  waits until it closes. The render then has to be prepared again.
- Checked in Chromium on a phone-sized page: lost in the Plan and never given back, lost with a
  render prepared and open, a render prepared again afterwards, the message after repeated
  losses.

Open: whether this was the owner's phone's failure: if the lamps still don't show, the next
step is the phone's own console (Safari's Web Inspector).

## 2026-11-21: the Nutcracker (recipe-developer)

**Why:** the owner made a nutcracker as a Lamp project (an 80 mm threaded tube in a closed cap) and
asked for its own template: only the cap and a key that screws in, the tube flattening above the
cap into a large key's handle.

- `nutcracker.easycad`: two prints (cap, key) from one set of settings, general steps only (the
  salt shaker's way: sections lofted, `contour`, `spiralize`, a ring-fill bottom). The key's tube
  flattens over its own length (S-curve or straight) into a stadium handle, as long round as the
  tube by default, two lines thick (solid) by default. Checks: a key too short for the cap, a
  flattening leaning past 60°.
- In the template list (and the offline cache), tests in `nutcracker_tests.rs`, doc
  [nutcracker.md](nutcracker.md).

Open: a test print.

## 2026-11-21 (later): the Plan's fittings can be hidden (claude/lampshade-light-render-wcvx6x)

A bulb button under the Plan's zoom buttons hides the socket, bulb and cable there and shows
them again (remembered on the device; the plan's PNG follows it). `scripts/render-test.cjs`
checks it.

## 2026-11-21: the Render view: the lamp's light, path traced; socket, bulb and cable (claude/lampshade-light-render-wcvx6x)

**Why:** the owner wants to see how a bulb's light works with the shades (bouncing off solid
ones, through see-through ones, bent by the layer lines), with the light on and off, as PNGs
to put on environments afterwards ([rendering.md](rendering.md)).

**What landed:**
- **The fittings** (`web/js/fixture.js`, `fixture-mesh.js`): for every recipe with a `socket`
  setting, a standard holder (E27 40 × 55 mm, E14 28 × 45 mm, custom from the recipe), a frosted
  bulb (A60 pear on E27, P45 round on E14) and a 6 mm cable straight up, in the 3D view and the
  Plan. Placed on the part whose beads run round the holder; the socket always points down, its
  face at that section's lower end as the lamp hangs. **Hangs ↓ / ↑** turns the project over
  (kept with the custom colours, in the file too; the recipe untouched).
- **The Render view** (`web/js/render.js`, a fifth button by Plan): **Prepare** takes a snapshot
  (toolpaths swept into solid beads, the fittings, the custom colours, the hidden parts) and
  builds the BVH, once; it lights up when the project has changed since. Then it path traces
  (three-gpu-pathtracer 0.0.27 with three-mesh-bvh 0.9.18, MIT, vendored, loaded only when the
  view opens): light on (a point light in the bulb, lumens and kelvin, the glass glowing, a little
  room light) or off (sky and floor light only); each part's custom colour and solidity as its
  material (solid bounces, see-through transmits that much and refracts through the beads);
  matte, exposure, samples, bounces, resolution. Its **PNG** has a transparent background.
- **One solid wall:** the faces where a turn sits on the next (`web/js/wall.js`) are rendered
  as inside the plastic (the same refractive index both sides), so light crosses the wall once in,
  once out, bending at the beads' round faces. With every bead its own tube, a see-through white
  shade came out dark brown even at 256 samples (light trapped in each turn).
- **The 3D view's PNG** leaves the bed grid out.
- `beadGeometry` moved out of `createViewport` (exported, shared with the render).
- Tests: `scripts/fixture.test.mjs` (sizes, placement, hanging, outlines, settings read back; in
  CI); `scripts/render-test.cjs` (the views and a render in headless Chromium, light on and off,
  the PNG's alpha; not in CI: on a software GPU a 16-sample render of the Lamp takes minutes).

**Open:** a denoiser, the user's own environment (HDRI) for the light off, the light on a wall or
ceiling as its own layer, the BVH built in a worker (preparing blocks the page), the 3D view
shown hanging too.

## 2026-11-20 (evening): the robot arm: nozzle tilt in the toolpath and a KUKA program (claude/amazing-newton-9qraf8)

**Why:** the owner wants the Sphere (the Thread pendant) printed on the KUKA KR50 of the SPHR
handover (`SPHR_Full_Print_Handover-2/`), with the nozzle along the wall instead of
compensating overhangs for a Cartesian printer.

**What landed:**
- **The nozzle's direction in the toolpath** (`Toolpath::axis`), and the step
  `tilt(path, max, smooth, bed, bead)` (`tilt.rs`): up the wall from the bead underneath,
  averaged over `smooth` mm, at most `max`, growing from upright on the bed; each move's bead
  becomes its gap along the nozzle (layer height ÷ cos lean × cos(lean − tilt)), so the
  compensated wall gets its full bead back. The view's nozzle shows it; stats give the most tilt.
- `wall::up_the_wall` finds the bead underneath on a hand-built line with neither turns nor
  layers (`thread_shade`'s) too: the nearest earlier extruding move a quarter to three layers
  lower within two widths across. Before, such a line was all upright.
- **The KUKA program** (`krl.rs`): the handover's start, purge, spline settings, triggers and end;
  poses in the base frame with the handover's roll; speed per move (flow ÷ bead, at most 25 mm/s,
  wrist at most 30 °/s); screw command per mm³/s; points left out within 0.2 mm; SPLINE blocks of
  1000; a test print's stop height. Refuses travels, machine events, a nozzle past 60°, points
  off the bed. A ZIP of `.SRC` and `.DAT` from the Print tab (printer **KUKA KR50**).
- **A hand-checked fixture** (`fixtures/hand-checked/robot-square.src`, worked out in
  `scripts/hand-checked-fixtures.py`), and the owner's Sphere on the robot as a test recipe
  (`fixtures/recipes/sphere-robot.easycad`): its first 40 mm make 1,621 spline segments, about
  an hour at 60 mm³/s.
- The Thread pendant template's **Robot** group (off by default: nothing changes for the G1).
- **`scripts/robot-check.py`:** replays a program on the handover's robot model (its
  `kinematics.py`, `proxy_collision.py`, CAD proxies and fan housings): joint limits and reach,
  tool/arm/pedestal collisions, 10 mm bed clearance, and the tool against the material already
  printed (the program's own bead). Its first run showed the nozzle fans 7 mm off the bed in the
  Sphere's 40 mm test (full tilt at 45 mm up was too soon): the tilt now reaches 45° at 60 mm
  (fans 25 mm off the bed); the test then passes every check.
- **`scripts/robot-viewer.py`:** the program animated in the handover's own viewer (its
  `viewer_template.html`), from the same kind of replay.
- **The whole Sphere** showed the wrist (A5) up to 5.7° past its limit in 1,249 poses, in the
  closing top where the nozzle leans in (the 40 mm test never got there). Now the writer works
  every pose out on the handover's kinematics (`kuka.rs`, ported and tested against
  `kinematics.py`), **rolls the tool about the nozzle** where a joint would come within 10° of
  its limit (smooth, at most 1.5 °/mm), and **eases the tilt back** where even that isn't enough
  (the handover's "local tilt relief"); it refuses a program with any joint closer than 3°. The
  Sphere: 9,483 points rolled (up to 90°), 4,968 with the tilt eased (4 % of the line), every
  joint at least 3° inside, confirmed by the handover's Python; 49,641 spline segments, 14.9 h at
  60 mm³/s. A4 is counted continuously (it reached −192° once rolled, which the wrapped angle hid).

**Open:** run it on the controller (it must compile: `C_SPL`, 1000 segments a block); the
checks in the engine (they're a script for now); calibrate the screw; travels and machine events on the
robot ([roadmap.md](roadmap.md), Open).





## 2026-11-20 (evening): Thread pendant: no lip above the grip, a rounded corner at the wall (recipe-developer)

**Why:** the owner's half sphere, after #55: a lip at the top floating over the arm under it,
and the arm leaving the outer wall in a sharp corner, almost straight down.

**What landed** (`pendant.rs`):
- A bug: each point followed the shade's profile at its own height, so an arm far below the
  wall's heights (the wings compensated on their own) took the sphere's radius further down,
  wider: pushed out against the wall, it dropped up to 60° there, and the crease above the grip,
  sitting where the arm should have been, floated 9.5 mm over it. Now by the wall's height.
- The arm leaves the wall level and curves into its climb over its outer 30 % (`bridge`, a
  parabola): no corner; the rest about 18 % steeper (Sphere20test's arm 15.8° at most, was 14°).
- Test: `the_arm_leaves_the_wall_level_and_the_crease_sits_on_it` (the half sphere; it fails on
  the old code).

**Open:** a test print.


## 2026-11-20 (later): Thread pendant: the wall and the wings compensated each at its own rate (recipe-developer)

**Why:** the owner's sketch and Sphere20test: since #54 the whole layer took the wings' rate, so
the sphere's wall was squeezed flat at the equator where it stands upright. They want the wall at
its rate, the wings at theirs, the arms (the bridges) changing angle between them.

**What landed** (`pendant.rs`):
- The wall rises its own compensated layer, the wings theirs (radial lean); their heights part
  and the arm takes up the difference (its own part of the lead fading from each layer's middle
  ring, the thread's from the socket's). Scaled back only if the wings couldn't finish the grip.
- The arm's blend is now a straight line (no steeper anywhere than on average): on the owner's
  sphere the leaf's arm 14° at most instead of 23°.
- With a crease, the lead left above the grip closes faster, as fast as leaves each crease layer
  75 % of its height (from the crease's share of the arm).
- A new check: the arm's layers over 1.5 times their height (info 23), where the wings move in
  far below the wall's heights (a straight arm whose low end moves in sits high on the one below).
- `thread_shade` refuses gaps (and the crease) over 1000 mm: a fuzzed 4 × 10⁹ mm wall gap ran
  out of memory. Tested.
- Measured, the owner's files: Sphere20test 147 layers, the wall a whole layer at the equator;
  the arm over 15° on the 70° sketcher wings and the half sphere (they warn), its tallest beads
  2.7–4.4 mm where the wings move in fast.

**Open:** whether to limit how far the wings may part from the wall (trading their own
compensation for a gentler arm), and a test print.


## 2026-11-20: Thread pendant: the leaf's pocket closes with the inner ring (recipe-developer)

**Why:** the owner's half sphere: closing the gap before the socket, the middle ring (joined to
the arm) swung in onto the inner ring, a harsh overhang where the layer height, the width and the
thread are all changing.

**What landed:** with the leaf the middle ring keeps to a straight line from the wall to its
place on the socket, the inner ring the gap inside it; the gap opens and closes by the inner
ring moving. On the half sphere the middle ring's steps stay 3.1–3.5 mm while the inner ring
slows to 1.1–2.6 mm (it was 5.6 for the middle one). The sketcher's wings unchanged. Tested.

## 2026-11-20: a bigger 3D view on the phone (claude/eager-maxwell-ph399i, the general manager's files with the owner's leave)

**Why:** the owner: on the phone the 3D view was a strip; they want it about half the screen,
pinned, the settings scrolling under it.

**What landed** (`web/styles.css`, phones up to 700 px wide): the view itself 38 % of the
screen's height (at least 180 px); its controls tighter (smaller buttons, the colour sliders side
by side in one row); a smaller header. On an iPhone-sized screen (664 px seen) the view went from
92 px to 252 px, its controls from 205 px to 130 px; the settings scroll in the remaining 209 px
under the pinned view and tabs. Desktop unchanged. Smoke and agent tests pass; checked with the
owner's half sphere.


## 2026-11-19 (night): Thread pendant: the wings compensated for their radial lean, even along the leaf (recipe-developer)

**Why:** the owner: on the sphere the leaf (one straight thing) had different layer heights
along it, and the lean to compensate is the wings coming in towards the socket, not the leaf's
tips growing. And in their straight file compensation seemed to change nothing.

**What landed:**
- The wings' lean is radial only (the rings coming in or going out).
- Where the wings run, the wall and the wings both rise the thinner of the two (each at least as
  compensated as it needs), eased in and out at 0.04 mm per mm: the leaf's layers even (tested),
  no drift between the wings' heights and the wall's, so no arm climb for it. This replaces the
  balanced stretches and the 7° drift of the evening; the lead is the thread's alone again.
- The gap under each point averages this layer's rise and the one below's (a ring on the socket
  came out 2.525 mm where neighbouring layers differ).
- Info: item 21 is the thinnest gap in mm again (old recipes compare it with half a wall layer);
  the share of its layer is item 22, which the template checks.
- The owner's straight file: 171 layers with compensation, 165 without (it changes now; if the
  app showed no change it was the version before #53).


## 2026-11-19 (late): Thread pendant: the wings compensated on their own, and the leaf (recipe-developer)

**Why:** the owner's sphere: the wings and the outer shape lean differently, so each needs its
own compensation, the arm taking up the difference ("if we need more layers on the inside they
have to come from more layers from the outside"). And: no shaping the wings with the max
overhang; a graph for the leaf's silhouette (against height), the path from where the wings
start to the socket.

**What landed** (`pendant.rs`, the template):
- **The wings' own compensation:** each layer's rise for the wings from their lean (the rings
  coming in, or the leaf's edges, whichever is steeper); the wall's from the shape's. Over each
  stretch of wings both get the same number of layers: whichever needs fewer gets thinner ones
  (on a sphere the wall gives the wings layers: 187 instead of 131). In between their heights
  part only as far as the arm can climb at 7°, back to none by the socket. The lead is now two
  parts: the wings' own (fading along the arm from each layer's middle ring) and the thread's
  (from the middle ring on the socket, as before), so the thread stays a true helix (tested).
  The wings and the arm are extruded for the gap they're in; the thin-layer check is now
  relative to each layer (under half warns).
- **Wing shape: leaf** (default) or **sketcher** (sketcher files open so). The leaf: the wing
  curve is its width against height along the zone, exactly; a straight path from the wall where
  the zone starts to the socket; no max overhang (the half-a-line check stays, by eye). The max
  overhang, path and path height show only for the sketcher's.
- Measured: the owner's sphere (leaf) ±9 mm, 10°, nothing under 69 % of its layer; the Angled
  cone with a 3.2 × 1.5 mm wall now warns of a 16° arm (the thread far ahead and the 70° wings).

**Open:** a test print of a leaf sphere; adding or removing leaf points on the graph itself.

## 2026-11-19 (later): Thread pendant: wing gaps that open from none and close on the socket; the bead changes along the arm (recipe-developer)

**Why:** the owner's photo: the first turn at each wing tip hung a whole line off the slot
under it (with a 5 mm gap it was 10 mm across). They want the gap to open from none, as fast as
it can, and none on the socket ("closed pockets"). And, from their view of line widths: the
change from the wall's bead to the thread's should happen along the straight arm only, every
ring in the grip at the thread's bead.

**What landed:**
- **The wing gap** opens from none: the first turn is a line across (half a line overhang). It
  grows by what's left of half a line after the inner ring's own move each layer, and (pockets,
  `thread_shade`'s `pockets`, on by default) closes again before the grip, as fast as the max
  overhang allows on top of the inner ring's move, planned so it only opens as far as it can
  close in time. The wing-step check now counts the middle ring too. Tested.
- **The bead and the thread's height change along the arm only**: from the middle ring on the
  socket (both rings and their turns at 5 × 2.5 mm in the grip; before, the middle ring was
  4.95 × 2.47) out to the wall. On the owner's straight shade the arm climbs 19° at most (17°
  before).
- **A curve's own bead is checked** as a toolpath's width and layer are (0.05–20 mm,
  0.02–10 mm): a fuzzed 100 m wall line hung the checks after it; now `toolpath` says which
  point. And `thread_shade` refuses a line over 20 mm.


## 2026-11-19: list settings: rows in between, and a graph to drag the points on (claude/eager-maxwell-ph399i, the general manager's files with the owner's leave)

**Why:** the owner: adding a row only at the end and shifting the values down is slow; they
want rows put in between, and the points of a curve dragged on a graph, like Grasshopper's Graph
Mapper, without recomputing while dragging.

**What landed** (`web/js/settings.js`, new `web/js/curve-editor.js`):
- **A "+" on every row but the last** puts a row under it, halfway to the next, value by value
  (as many values as both have, so a corner flag isn't copied). "+ Add a row" still copies the
  last row at the end.
- **Table | Graph** above any list of points ([x, y] or [x, y, corner]); the choice is
  remembered per setting on the device. The graph draws the points on a monotone cubic through
  them (as the Thread pendant reads its curves; a corner splits it), axes 0..1 when the values
  are, else their span. A point drags between its neighbours along x and inside the axes, rounded
  to a thousandth of the axis; nothing is computed while it moves, letting go is one change
  (written into the recipe as typing is). Both views edit the same rows.
- Tested: the in-between row and the graph's helpers (`scripts/settings.test.mjs`), the smoke
  test puts a row in between and removes it; checked in Chromium (desktop, phone): a drag leaves
  the shade alone until let go, then the recipe has the new point.

**Open:** adding or removing points on the graph itself. The request test (not in CI) fails
waiting for "Send to be built", with or without this change.

## 2026-11-19: Thread pendant: layers rise a layer a turn again, no seam (recipe-developer)

**Why:** the owner's print: flat layers (the default since 17 Nov) step up to the next layer
over the last two line widths at one place, a seam straight up the whole wall, far too visible.
They want every layer rising a layer a turn.

**What landed:** "angle" layers (every point at its angle's height, as the sketcher) are the
default again: in the template, in `thread_shade` without `z_by`, and for sketcher files. The
wall rises steadily all the way round and never steps (tested: nowhere faster than a layer a
turn, a whole turn exactly its layer, with compensation and a thinner wall); the thread on the
socket is the same true helix as before, rising a pitch a turn while the wall rises its own
layer, the wings climbing between them. "flat" stays as a choice. A project saved with
"flat" keeps it: set Layers to "angle".

## 2026-11-18: Thread pendant: a thinner wall and wings than the thread (recipe-developer)

**Why:** the owner wants thin layers and lines in the shade and the wings, to save plastic, and
the socket's pitch and a thick bead only on the thread. (Done with the owner's leave in the
general manager's files too: a curve that carries its own bead.)

**What landed:**
- **Curves can carry a bead** (`Curve::bead`: a width and layer height per point), and
  `toolpath` prints each move for it (the per-move width and height the toolpath already had,
  as `bead` sets them). Moves and turns keep it; a tween or anything that remakes the points
  drops it. Tested.
- **`thread_shade`: `wall_line`, `wall_layer`** (0: the thread's). The wall and wings print
  with their own bead; the thread grows into the pitch and its own width over the layers before
  the grip (a tenth of a pitch a layer), a true helix on the socket, and back after. The line
  carries each point's bead, the first layer's for the gap over the bed (the template's `bead`
  step is gone).
- **The lead is shared**: the thread starts half of it below the wall and ends half above (all
  below with full wings), so the bridge climbs half as far. This changes yesterday's compensation
  too (the thread started level with the wall's top then): the owner's cone now runs ±3.3 mm.
- **Widths follow heights across the bridge**, so the middle ring next to the thread is as wide
  as its pitch-tall layers. The wing rings' sideways step is limited by their own rise.
- **Two checks:** the bridge climbing steeper than 15°, and the wing rings' layers thinner than
  half the wall's (new info items 20 and 21).
- **Too big is said at once:** a shade with millions of points used to be built before the
  error (a fuzzed 4 × 10⁹ mm radius took a minute); the outer wall's points are counted first.

The owner's Angled cone, 3.2 mm × 1.5 mm wall and wings, the E27 thread 5 × 2.5: 232 layers,
2,004 cm³ and 28 h instead of 2,980 cm³ and 41 h. The thread is ±11.6 mm from the wall, the
bridge climbs at most 12.5°, and no warning besides the 70° wings' (as before).

**Open:** a test print, and which nozzle: a 3.2 mm wall wants the 3 mm one (the width check
passes 1–2 × the nozzle).

## 2026-11-18: Thread pendant: compensation on the wall, the pitch only on the thread (recipe-developer)

**Why:** the owner's shades with more overhang: the compensation stopped a layer below the grip,
so from there to the top every whole layer (the outer wall too) rose a full pitch. Only the part
on the socket needs the pitch.

**What landed** (`pendant.rs`, `layer_plan` and `finish`; the default, no setting):
- The outer wall is compensated in every layer, bottom to top.
- The thread on the socket rises exactly a pitch a layer, a true helix as before (tested in
  flat and angle layers, and the half sphere's every point on the barrel).
- The thread sits below the wall in the grip and meets it at the top. Below the grip it rises a
  little less than the wall, then eases into the pitch over 4 layers, and back out over 4 above.
- In each layer the slot and wings blend the two heights by radius, so the line climbs gently
  from wall to socket and back. The extrusion is unchanged: solid, a whole pitch's bead.

The owner's Angled-cone file: 139 layers instead of 134, and the wall 2.13 mm all the way up.
The thread is 2.5 mm on the socket and at least 1.88 mm below it. The two are up to 6.3 mm
apart, and the bridge climbs at most 1 in 8.5.

**Open:** a test print. In `nodes.rs` (the general manager's), the `thread_shade` description
still says "below the thread".

## 2026-11-17: works offline again, every file (general-manager)

**Why:** the owner asked whether the app works without WiFi. It does (the engine runs in the
browser; only the Chat tab needs the internet), but the offline cache's list in `web/sw.js` was
written by hand and had fallen behind: the Plan's code (`js/parts.js`), the sketcher, and the
Lamp and Thread pendant templates were missing, so offline they worked only if used online first.

**What landed:** the missing files are on the list, and a test (`settings.test.mjs`) fails when a
shipped file (code, template, icon, vendored library) isn't, naming it. Checked in Chromium:
opened once online, then offline: reloads, the engine starts, the Lamp and Thread pendant open.
A new template or script must be added to `web/sw.js` (the test says which).

## 2026-11-17: Thread pendant: flat layers, and no zigzags where the rings start (recipe-developer)

**Why:** the owner's first look (photos of the toolpath): little zigzags by the wings near the
socket, and the whole layer rising when only the part on the socket should.

- **Zigzags:** the sketcher draws each slot straight in its own frame, so its ends landed
  0.2–0.4 mm outside the wall and the rings (most near the socket, the smallest radius), then the
  ring started on its own radius: 876 zigzags in the straight shade. The slots now land on the
  radii (tested: none, either way of layering). The sketcher's points are within 0.5 mm of ours.
- **Flat layers** (`z_by: "flat"`, the default; the sketcher's "angle" opens as flat): every layer
  level, only the thread a helix (the inner rings in the grip), easing in and out over 4 layers
  each side (all exactly a pitch); each layer steps up along its last two line widths at the seam.
  "angle" (all helical, the sketcher's) and "length" stay as choices. Tested: the wall is level
  away from the seam, the thread a true helix.

## 2026-11-16 (late): the Thread pendant, from the Thread Lampshade Sketcher (recipe-developer)

**Why:** a friend of the owner's built a Claude artifact, the Thread Lampshade Sketcher: wings
that run in from the wall and close into rings that thread onto the socket. The owner: make its
pendant here, printable, reading its settings files natively, E27 (E14 later); the general
manager's part allowed for this one.

- **`thread_shade`** (`crates/core/src/pendant.rs`): the sketcher's generator ported to Rust,
  pendant with the wings on the socket; the same points as the sketcher's own for its four files
  (to 0.00002 mm, the browser's floats; a test keeps samples of two). A project's own step, against
  "general steps" on purpose: it keeps the files and the shade the same.
- **Printable:** the first layer on the bed (`bead`), overhang compensation by layer height below
  the thread (never in it: the grip stays a true helix, tested), line width and flow as settings,
  the sketcher's checks as warnings plus one: wings stepping more than half a line a layer (its
  65–70° files do, at 6.9 mm a layer).
- **The template** `thread-pendant.easycad` (the sketcher's settings by name), in the template list.
- **The files:** Open or Load settings takes a sketcher `.json` (`web/js/sketcher.js`, tested
  with its files in `fixtures/sketcher/`, `node --test scripts/sketcher.test.mjs`, in CI); "Save
  for the sketcher" writes one back.
- Found by the sweep: a custom pitch rounded the first point a hair under the bed (fixed, tested).

Open: a test print; the E14 barrel; a pace for the wing rings; floor lamp, linear pendant and
screw part not made ([thread-pendant.md](thread-pendant.md)).

## 2026-11-16 (night): the wings' outline follows the % smoothly (recipe-developer)

**Why:** the owner's sphere (eight seams, a curve down to none): "always that pointy bit that
doesn't follow the curve", at every setting. It was the recipe: while a wing formed, the opening at
the seam rushed to its full width, then the tips waited there until the turns were half circles
(in that file: 5.5° for four layers), then ran on: a stall and a jog in the outline.

- Now the tips reach round in step with the % from the first layer; the opening widens with them
  up to the seam's angle, the turns form under them over about twice that (slowly at first, so the
  opening never needs more than the reach). The middle wall sits up to a tenth of a line over the
  wall for a few layers while a turn forms (the growth test allows that, full wings stay strict).
- No button needed: it's how every wing forms now. The tips' pace counts each level's forming
  over its own radius (a deeper level's was counted too fast).
- Test: the owner's sphere, every growing layer's tips within 20 % of the % (they stalled at 0).
  The fit-the-shape test skips layers under 3 % of wings and a bell's last two (lengths every two
  layers are too coarse where it flares flat).

## 2026-11-16 (evening): wings tried on every shape: a pace, room past 100 %, a choice of start (recipe-developer)

**Why:** the owner: the fade-in couldn't be found, wings past 100 % didn't work, "a few things
don't work". A scratch survey (every shape, in and out, 50–300 %, seams, gap, curve, fit, wide edge
down, lip) checked each print for beads with nothing under them and lines crossing in a layer.

- **Fade-in hidden on every shape but the tube:** its settings named `wing_bed` (tube only), and a
  setting shows only when every setting it names applies. Fixed; the bed start now applies only to
  the tube in the recipe too.
- **Wings start: fading in / full** (new): full puts the wings at their set % on the first layer.
- **Tips grow at most ½ line a layer** (`wing_grow`, new; 0: no limit): grown faster, wings on a
  cone or sphere hung in the air at their tips (up to 26 mm gaps) and their forming sides spread
  ~5 mm a layer. Now every bead in the survey has one under it, apart from the bell's and dome's
  own flat tops. Warns when the wings can't reach their peak at that pace.
- **Past 100 % inside:** levels by the room at each layer's radius (was an error on any E27 shade).
- **Fixed:** a level not started yet with its radius below zero drew a loop and a whole circle at
  the seam (cone 300 %); dividing by zero with no pace limit; a curve wide edge down starting at 0
  instead of its value; outside wings on a widening cone crowding the next layer (each point now
  follows the shade's radius at its own height); the dents warning on wings the room or pace capped.
- Tests: the pace (no bead hanging, inside and out; hanging without it), the full start, deeper
  wings on a wide cone (no crowding), 200 % inside on E27 (a warning, 100 % printed). The wing
  tests of other features run without the pace.

Open: outside 200 % with "same layer length" widens the tube from 23 to 144 mm over the fade-out
(a 60° flare; flipped, unprintable): maybe cap it. Tips on a narrowing top (sphere, in-out) still
overhang a little. Default pace (½ line) to check in a print.

## 2026-11-16 (latest): wings end on a whole layer, and grow without a kink (recipe-developer)

**Why:** the owner's cone (wings outside, four seams, a curve from 100 % at the throat down to
20 %, wide edge down) had one odd loop next to the throat, and a kink in the outline where the
wings went from growing round to growing along.

- **The loop:** each layer read its wings at its middle, which for the last one was half in
  the plain throat: 57 %, between the full wings and none. Now each layer reads them inside the
  wings' own stretch, carried on along the curve's slope past its last grid height (never up),
  so the last layer by the throat has 99.6 % and a bell's last layers still fade to ~2 %.
- **The kink:** a tip grew by its turn's angle up to a point, then by the tip's angle: two
  slopes meeting. The two now blend over half a turn either side (a quadratic), so the tip's
  angle has no corner as the % changes.
- A test with the owner's settings: the last winged layer ≥ 95 %, every winged layer 20–100 %,
  no jump over 0.12 between layers.

Open: nothing new.

## 2026-11-16 (later): Wings keep "wide edge down" in sight (recipe-developer)

**Why:** switching Wings on hid "Wide edge down": it shows while the shade isn't closed, and
Wings hid "Closed end" (a setting only shows while the settings its `when` names show too).

**What landed** (Lampshade and Lamp): the other way round: "Closed end" always shows and "Wings"
shows while the shade isn't closed (a closed end prints disc down, wings or not; closed, the shade
has no wings). Checked in the app: with Wings on, Wide edge down shows; closing the shade hides
Wings and Wide edge down.

## 2026-11-15 (evening): overhangs keep their plastic (general-manager)

**Correction:** the roadmap's overhang note suggested extruding less on squeezed turns and a floor
on the rise. The owner doesn't want either: every turn keeps the same plastic for even layer
lines, and the compensation is only how close the turns sit. The note now says so, with tests 5
and 6 (test 6: 80 %, steep 0.8, pivot 76, opening 160 mm).

## 2026-11-16: wings are a setting of every shape (recipe-developer)

**Why:** the owner wanted wings and shape set independently: any shape, wings on or off like the
brim, and the wings following the fades, a curve of their own, or the shape.

**What landed** (Lampshade and Lamp, [tube-with-wings.md](tube-with-wings.md)): the shapes are the
plain ones again (no "tube with wings", "cone with wings"…; old projects pick their shape again
and switch Wings on). "Wings" at the end of the Shade group switches on a "Wings" group: "Wings
follow" fades (the %, faded in and out; the tube can widen to keep the layer length), a curve
(`wing_curve`, rows of where along the shade 0..1 and %, a smooth curve through them; the fades
are not used), or fit the shape (as long as the widest layer). Inside/outside, seams, gap, rise,
start from the bed (the tube) as before. Tests `every_shape_can_have_wings` (9 shapes × 3 modes)
and `the_wings_can_follow_a_curve_along_the_shade`; the earlier tests on the new settings.

## 2026-11-15 (night, last): the widening follows the fade-out's curve; points per arc (recipe-developer)

**Why:** the owner's outlines: where the wings ran out, the wall flared hard right beside their
pointed tips. With the wings driving the radius, the wall barely widened while the wings shrank
slowly and then jumped out in the last stretch, as the turns flattened into dents.

**What landed** (Lampshade and Lamp): with "same layer length" the tube's radius now follows the
fade-out's curve from its own radius to the plain circle as long as a full-wings layer, and the
wings are as much as keeps each layer that long (the same table as the shapes with wings). And
every arc gets as many points as the chord tolerance needs at its own radius and sweep (one when
there's nothing to draw): the template as it ships drops from 9,100 points to 5,300, the owner's
259 % tube from 115,000 to 52,000. Test `fading_out_the_tube_can_widen_to_keep_each_layer_as_long`
now checks the wall half way out half way up a straight fade-out.

## 2026-11-15 (night, later): a widening tube no longer narrows where a level of wings ends (recipe-developer)

**Why:** the owner's tube (from the bed, 259 % outside on 6 seams, widening as the wings fade out)
narrowed in two places on the way up. Made before today's exact-length fix, it still guessed the
layer lengths, worst where a level of wings runs out; with the fix it was just over the engine's
geometry budget (115,000 points through ~20 list steps each).

**What landed:** the points are built with fewer list steps (each coordinate in one `flatten`),
so it fits; its wall widens all the way (16.5 to 87 mm). Test
`a_tube_widening_through_three_levels_of_wings_never_narrows`. With outside wings the outermost
level's walls end where the wings drop below 200 %: the outline steps in there by about two walls
(that's the wings, not the wall).

## 2026-11-15 (night): from the bed, the wings screw onto the socket (recipe-developer)

**Why:** the owner: with full wings from the bed (no straight start), the socket screws into the
wings themselves, so the innermost wall must fit the socket.

**What landed** (Lampshade): from the bed, inside wings put their innermost wall at the socket's
thread (the tube's wall two walls further out per 100 %); outside wings keep the tube's wall
there. A warning when the rise can't make that wall a thread (inside: rise along the wings at a
whole 100/200/300 %; outside: rise along the tube wall). Test
`from_the_bed_the_wings_screw_onto_the_socket` (both sides: the wall at the thread's radius, a
helix a pitch a turn from the first layers; the warnings).

## 2026-11-15 (evening): cone, bell, dome, sphere and flat sphere with wings (recipe-developer)

**Why:** the owner wanted the shades with wings too, but with the shape set first and the wings
following from it, rather than the % driving the shape.

**What landed** (Lampshade and Lamp, [tube-with-wings.md](tube-with-wings.md)): five more shapes,
"X with wings", each the shape X with wings that keep every layer as long as its widest
(at most the Wings %, faded in from the plain throat, stopping at a lip or straight end). The
same table that now works out the tube's widening works out the wings: per height, at 12 amounts
of wings, the amount that gives the widest layer's length. Tests
`a_shape_with_wings_keeps_its_layers_as_long_as_its_widest` (all five: within 3 % once faded in,
never over; the throat plain; wide edge down) and `a_shape_with_wings_is_a_shade_too` (a sphere
with wings in the Lamp, a straight end that takes a cone, no wing ports).

**Open:** the Plan's add buttons give new shades sizes by their shape's name (`web/js/parts.js`):
"cone with wings" etc. come with the component's defaults.

## 2026-11-15 (later): "same layer length" works out each layer's real length (recipe-developer)

**Why:** the owner's flared tube (8 seams outside, a 10 mm gap) bulged and necked where its wings
ran out: the tube's radius grew, shrank, grew again. The length of a layer while its turns
form was guessed (the full turns' length scaled down), far off with a gap; and with no fade-in
the first height had no wings (0 ÷ 0), its radius leaking into the next.

**What landed** (Lamp and Lampshade): a layer's exact length (partial turns and all) is worked
out every two layers up at 12 radii and the radius that keeps the longest layer's length is read
off between them, no Newton steps; "fade in over 0" is full wings at once; the widening starts
exactly where the fade-out does; the wings start on the straight start's end even when they
start wider (no hop). Test `a_flaring_winged_tube_widens_smoothly_to_the_end_of_its_wings` (the
owner's settings: the wall only widens, no bunched layers, one line).

## 2026-11-15 (later): STEP is the surface, not a thickened wall (general-manager)

**Why:** the owner works on exported shapes as surfaces; a wall thickened to the line width was
the wrong thing to hand over. STEP doesn't need a solid: a surface goes in as an open shell.

**What landed:** the Lampshade and every Lamp shade export `wall` itself (the surface through
the bead's middle) as STEP: one open shell of exact cones, cylinders and planes, no solid, the
sizes as designed (the default lampshade 150 mm across, was 156 thickened). `thicken` stays as a
step for a recipe that wants a solid. Checked with Open CASCADE 8: the default lampshade, a
closed bell in-out, the tube with wings and a Lamp shade each read as one valid shell. Tests and
the smoke test expect the surface model; docs and the AI's guide updated.

**For the recipe developer:** the tube with wings' `wall` is the plain tube, so its STEP has no
wings (on the roadmap).

## 2026-11-15: `when … and …`, a hand-built spiral's layer count (general-manager)

**Why:** the recipe developer's "Needs from the app" for the tube with wings: settings that hang
off a hidden one stay shown, and the app counted the wings (one long curve) as one layer; and a
Wings group of its own would have failed the smoke test.

**What landed:**
- `when A and B and …` on a `param` line: every condition must hold (each may have `not` and
  values), and every setting named must apply itself, as before. Parsed into `Condition.also`;
  the settings window, the engine's "no setting" warning, parts (names made the part's) and the
  STEP header's list of settings that apply all read it. Docs and the AI's guide updated.
- `toolpath(…, layers: n)`: how many layers a toolpath prints, for the count (0: one per curve).
- `join` doesn't count a print aid's layers (`aid`, a brim): the count is the object's.
- The smoke test checks the settings groups that show for the cone it opens with, so a shape's
  own group (hidden for a cone) can be added freely.

Tests: `a_when_can_join_conditions_with_and` (lang), `layer_counts_for_hand_built_spirals_and_brims`
(core), the `and` case in `settings.test.mjs`; the open-top golden test finds the last turn by
its turn number (with a brim the layer count is now one less). Golden G-code unchanged.

## 2026-11-15: a tube with wings prints wide edge down too (recipe-developer)

**Why:** the owner flipped a winged tube in the Plan (it hangs down from the socket, widening at
its end) and it still printed start first.

**What landed** (Lamp and Lampshade): "wide edge down" works for the tube with wings: the whole
tube end first from the bed (the schedule of the wings read from the other end, the first layer
rising off the bed through `bead`, a brim round its outermost wall), its straight start last;
shown in the same place in the lamp, its ports where they were. Tests
`a_tube_with_wings_can_print_wide_edge_down` (the owner's tube: on the bed at its widened end,
plain thread at the top, the same radii at every height turned over, the same span in the lamp)
and `the_tube_can_print_wide_edge_down` (Lampshade).

## 2026-11-15 (late night): a winged tube takes a thread only where it rises a pitch a turn (recipe-developer)

**Why:** the owner: shades must only be added where they can actually thread on, and that
depends on how the wings rise.

**What landed** (Lamp): two more ports on a tube with wings, "Wings, outside" and "Wings, inside",
along its full wings (between the fade-in and the fade-out). Rising along the tube wall: the
wall's free face; rising along the wings: the deepest wings' far wall, only at a whole 100 / 200 /
300 % with full turns; rising along the whole layer, or while the wings fade: no port (length 0),
so the Plan offers no add button and the settings no such mount. The straight start and end keep
theirs. Test `wings_take_a_thread_only_where_they_rise_a_pitch_a_turn`; the table is in
[tube-with-wings.md](tube-with-wings.md).

## 2026-11-15 (night): the tube with wings in the Lamp; layer height compensation only (recipe-developer)

**Why:** the owner wants to plan winged tubes in the Lamp, threaded onto and into other shades;
and uses only layer-height overhang compensation.

**What landed:** the Lamp's shade has the shape "tube with wings", with the Lampshade's wing
steps and a "Wings" group: its own straight start screws on (its start ports only that long),
the wings fade out up to a straight end (or stop there), and shades screw onto that end at its
radius, wider when it widened. Test `a_tube_with_wings_is_a_shade_too` (`lamp_tests.rs`): the
ports, plain circles in the straight end, full wings below, a cone on the widened end with no
collision. Width compensation is gone from the wings (both templates; "width" prints them at the
line width) and won't be kept up; the shades' "width" option stays for EasyGCode's golden tests.

**Open:** the Lampshade and the Lamp now share the wing steps as two copies too.

## 2026-11-15 (evening): overhang compensation where the winged tube widens (recipe-developer)

**Why:** with "same layer length" the tube widens as the wings fade out, and its wall leans out:
an overhang, which the owner wanted compensated as the shades' are.

**What landed** (Lampshade, tube with wings): the Overhangs settings apply: "layer height" brings
layers closer where the wall leans (each still a whole layer's bead). The fade, the widening and the lean are now worked out on a fine grid of heights,
and the layers are laid along it by their own rise, so the shape doesn't move and every layer
that doesn't lean is exactly a pitch; the top ends on the nearest whole layer. As it ships (no
widening) the path is unchanged to the last digit. Test `a_widening_tubes_overhang_is_compensated`.

## 2026-11-15 (later): wings that part at the seam; long tubes past 100 % (recipe-developer)

**Why:** the owner's renders: a wing started as a wide hole that the lines then grew back across
(its turns sat at the finished seam gap from the first layer), where it should start at the seam
and open smoothly; and their 533 mm tube with two seams outside failed past 100 % ("series:
count must be between 0 and 100000").

**What landed** (Lampshade): the turns now start together on the seam and move apart only as far
as the two sides need not to cross, reaching the finished seam gap by a quarter turn, so the
wall parts at a point and the gap opens (never wider than it ends up). The points are made one
short `series` per piece, in print order from the table of arcs, about 12 list steps a point
instead of 30: the template as it ships evaluates in 26 ms (was 50), and the owner's tube prints
at 150 % and 200 % (112,000–124,000 points). The wing settings have a group of their own
("Wings"), and with `when … and …` (general-manager) the settings that do nothing for a shape are
hidden: Wide edge down for the wings, the straight start and fade-in when starting from the bed,
the fade-out's settings unless there's a fade-out. `toolpath(layers:)` gives the app the real
layer count (213 for the owner's tube, not 17). Tests: the crease starts within 3° of the seam
and opens; the owner's tube past 100 %.

**Open:** the first print; "same layer length" is within 4 % (closed form).

## 2026-11-15: wings that start as a dent, outside, past 100 %, from the bed (recipe-developer)

**Why:** the owner's corrections and additions to the tube with wings: a small wing made four
tiny half circles (the whole shape scaled down), where it should only dip in at the seam and
turn back; and wings outside (a thread inside), a rise in the wings only (inside wings with a
thread inside), more than 100 %, and full wings from the bed.

**What landed** (Lampshade, [tube-with-wings.md](tube-with-wings.md)): a wing now starts as a
shallow dent whose turns grow from a few degrees to half circles before the wings run back;
`wing_side` (inside, outside); `wing_rise` "whole layer", "tube wall" (was "outer wall"), "wings"
(the innermost wings' inner wall a helix, the tube's wall level), blended in as the wings fade
in; `wings` up to 300 % (a second wing, then a third, grows on the inner wall at the seam);
`wing_bed` (full wings from the first layer, which rises off the bed through `bead`; a brim goes
round the outermost wall). The recipe is now one table of arcs per layer, seam and level, and
each piece's share of the rise comes from its arc, so it's as fast as before (about 60 ms as it
ships). Tests (`blocks_tests.rs`, `wings`): only forward moves while the dent is shallow; no two
pieces closer than a line as the wings grow, at 100 % inside and outside for 1–3 seams, and at
150–200 %; the tube wall's helix ("tube wall") and the inner wall's ("wings"); from the bed; the
errors and warning.

**Open:** as before (the first print; "Wide edge down" shows; the app's layer count).

## 2026-11-14 (late night): the Lampshade's tube with wings (recipe-developer)

**Why:** the owner's idea, from a sketch: a narrow tube's layers are too short to cool. Wings make
each layer longer without widening the tube ([tube-with-wings.md](tube-with-wings.md)).

**What landed:** a new Lampshade shape, "tube with wings": above a straight start (the thread,
printed as the tube is today), each layer turns back on itself at its seams onto a middle and an
inner wall, a line apart (or a line and a gap), the tips meeting a line apart at 100 %. Settings:
wings %, seams (1–8), gap, rise along the whole layer or the outer wall only (a thread outside),
straight start, fade in (linear or smooth), full wings for, fade out (0: none; linear or smooth),
and the diameter as they fade out (the same, or the same layer length: the tube widens). Small
wings start as a bump at the seam that grows. General steps only, printed with `toolpath` after
`vase_revolve` through `join`; no other shape's G-code changed (golden tests green). Tests in
`blocks_tests.rs` (`wings`): the start joins the straight part on the seam; one line, never
going down; the wings grow from a bump; no two pieces of a layer closer than a line (and the
tips meeting at a line) for 1–3 seams and with a gap; the outer wall on the thread's helix when it
rises alone; the widening keeps each layer within 3 % of full wings' length; the errors and the
warning. The new shape is also in `every_lampshade_shade_prints_both_ways_up`.

**Open:** the first print (watch the small bump's extra plastic at the seam); about 60 ms an
evaluation (the other shapes 4–7 ms); up to about 800 mm of wings (the geometry budget); "Wide
edge down" shows but does nothing for it; the app counts its wings as one layer. The settings
are in the Shade group for now (a Wings group of its own would change the smoke test's group
list, `scripts/smoke-test.cjs`). Not in the Lamp yet.

**Needs from the app** (for the general manager): a `when` that takes two conditions (`when
shape = "tube with wings" and wing_out`), so settings that hang off a hidden one stay hidden;
a way to give a hand-built spiral (`toolpath` of one long curve) its layer count, for the HUD.

## 2026-11-14 (night): a clear-out for working live (general-manager)

**What changed:** the roadmap no longer plans phases: it says what EasyCAD does today, the
decisions that stand, one **Open** list split by owner (app / recipes), and what's for later.
The dev log's November entries moved to `devlog-2026-11.md` (their open items are on the
roadmap now), and `CLAUDE.md`'s phase history became a short "where we are".

## 2026-11-14 (night): two agents, two branches (general-manager)

**What changed:** the owner split the work between two agents on long-lived branches:
`general-manager` (the app and its architecture: engine, language, app UI, G-code safety, build,
CI) and `recipe-developer` (templates, the owner's projects, tuning and test prints). The old
session branch is gone. Written down in `docs/working-together.md` (who owns which files, how a
recipe asks for a new step, merging `origin/main` before every task and every pull request,
conflicts, dev log entries ending with the branch), `CLAUDE.md` (collaboration workflow, read
order) and the README.
