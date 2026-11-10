# Dev log

Newest first. Each entry: what landed, numbers, what's still open. Keep it short; it's how the
next session (human or agent) picks up.

October 2026 (from the first commit to 29 Oct) is in [devlog-2026-10.md](devlog-2026-10.md).

## 2026-11-10: the smoke test after the G1's origin moved to its bed's centre

**Why:** PR #18 moved the G1's recipe origin from EasyGCode's 125/125 to its bed's centre
(500/500). The smoke test compares the app's G-code for the spiral disc, thread connector and
lampshade with EasyGCode's fixtures, made at 125/125, on the pellet printer: every move came out
375 mm over, and the PR's CI failed (the smoke test runs on pull requests only, so `main`'s run
after the merge was green).

**Landed:** the smoke test sets the origin to 125/125 for its EasyGCode comparisons and puts
the printer back to its defaults after each. The G-code itself is as the PR made it.

## 2026-11-09: the Plan leads to the settings

**Why:** the owner asked to tap a piece of the shape (a fillet, the throat) and land on the
setting that drives it, in every project, not only the lamp.

**Landed:** `mark curve for setting, … "Label"` in the language (components too, where the
settings are the part's own). The Plan shows for any recipe with marks: each mark a dotted line
over the cross-section, a band to tap; the tap opens the Settings tab at the first of its
settings that shows (fold-outs opened) and lights them all up. The lampshade marks its throat
(throat, socket, fit), fillet, shade (shape and its settings), opening, height and lip; the
lamp's shade its start (length, mount), fillet, shade, opening, widest, height, straight end
and end fillet.

**Open:** the other templates (vessel, shaker, thread, stool, disc) have no marks yet; the
round ones fit the Plan's cross-section as they are, the stool and the disc would want a view
from the front or above.

## 2026-11-09: fillets into the straight end, a smooth surface, an open top by default

**Landed:**
- The lamp's straight end rounds in from either side: a shade that opens out into it (a cone,
  a bell flat at its rim) now gets the fillet too, the case the spheres had mirrored across the
  straight end. It used to come out negative and was left off: a corner.
- Surfaces of revolution are drawn straight from section to section (`Surface::straight`, the
  3D view only): the smooth curve the full-quality mesh drew through the profile's points
  overshot where a long straight span met short ones (a groove where the throat meets the
  shade, a ledge under the straight end) and snapped in after a slider let go. A profile point
  repeating the one before is left out.
- Lampshade and lamp end on an open spiral by default (no flat closing turn). The golden tests
  ask for EasyGCode's closing turn, its width compensation and its ramp.

## 2026-11-09: colours in the project file, and the view as a PNG

**Landed:**
- The custom colours travel with the project: a saved project file holds them in one comment
  line between its note and the recipe (`#~ colors {…}`: each part's colour and opacity, the
  light, and whether the view was in Custom), and the settings file in a `colors` field.
  Opening either puts them back (and the view in Custom, if it was); the recipe itself never
  holds them, so the engine and the agent don't see them.
- **PNG** (next to Fit): the view as it is on screen, drawn at twice the screen's resolution
  (at most 4096 pixels across), saved as `<project>.png`.

## 2026-11-09: custom colours for simple renders

**Why:** the owner wants simple renders of lamps made of several shades: each shade its own
colour, some see-through, some solid.

**Landed:**
- **Custom** in the colour list: a row per printed part (colour picker, see-through ↔ solid)
  and two sliders for the light: **Highlights** (how much white) and **Shadows** (how deep the
  black goes); ½ each is the view's usual light. Shapes (Shape mode: smooth surfaces for a
  render), beads and toolpath lines take the part's colour; shapes and beads its opacity.
  Remembered on the device per project and part name; the Plan uses the same colours.
- The engine: what a part shows inside its component (a shade's surface) belongs to that
  part, so the parts buttons hide it with its shade and it takes the shade's colour.

**Open:** the colours lived on the device only (done next: in the files, and a PNG button).

## 2026-11-09: layer height compensation by default; its own strength along the surface

**Landed:**
- The lampshade's and lamp's overhang compensation is **layer height** by default (it was
  width, EasyGCode's; the golden tests ask for width).
- Along the surface the compensation works again, with its own **Strength along the surface**,
  0 % to start with: the nozzle turned with the wall makes up for the lean. (The step before
  switched it off along the surface.)
- Checked with the owner's settings (a bell 400 mm open, sliced along the surface): every turn
  steps exactly 2.5 mm along the wall, every bead 6 × 2.5 mm, every turn starting on the seam's
  90°, one straight seam. The gaps in the owner's screenshot came from the version before
  (69 turns, the old step of up to a line width); the test now checks the seam too.
## 2026-11-09 (later): a review of `main`: the stool 25× quicker, settings that errored, two new tests

A look through the repo for bugs and dead weight, the bend stool and the lampshade first.

**Found and fixed:**
- **The bend stool took 2.2 s a run in the browser** (89 ms now; the recipe itself is 40 ms).
  Since 8 Nov every run worked out the way up the wall at each toolpath point for the view's
  Nozzle picker, upright or not; for the stool's 91,000 points of rings that's ten times the
  recipe. The engine now does it only when asked (`evaluate`'s `wall`; the app asks while the
  nozzle isn't upright, and runs once more when it's first tilted), and its search grid is sized
  per layer (one wide primer line made every layer's cells big): 478 → 170 ms natively.
- **Settings that led to an error inside their own range:** the stool's foam extrusion under
  10 % (its speed override would pass the printer's 1000 %; the range is 10–100 % now) and its
  primer line widths over 20 mm (a bead's limit; 0.4–20 mm now); the lampshade's and lamp's
  compensated line width over 20 mm (a 12 mm line on a steep wall; it stops at 20 mm now).
  Golden G-code unchanged.
- **G-code checks:** a move whose feed would be written as `F0` is refused (Klipper stops the
  print on it: a very low flow in a huge bead, or a tiny speed factor); the filament printer's
  nozzle and bed temperatures are checked like the pellet printer's (0–350 and 0–150 °C, as the
  Print tab allows). `join` refuses more parts than its 16-bit part numbers hold instead of
  wrapping round to another part's bead (65,536 two-point toolpaths; a made-up case).
- **A freeze:** `vase_spiral` on a wall of absurd height (1e300 mm: more layers than a number
  holds) counted its points wrong and then spiralled for ever; it says "too many points" now.
  Found by the new fuzz test.
- **Docs:** the README's status, `architecture.md`'s header (what's built, what's still a plan)
  and its module list.

**New tests:**
- `crates/core/tests/fuzz.rs`: every template and test recipe damaged (awkward numbers, lines
  dropped or moved, made-up step calls, stray characters) and run through evaluation, the
  simulation, G-code and STL; a panic or a hang fails. 40 cases with `cargo test`; a round of
  6,000 with overflow checks found the freeze above (4 cases); a second of 3,500 found an
  overflow in the wall direction's grid at a point 1e300 mm away (harmless as shipped, where
  it wraps to a miss; fixed) and nothing else.
- `crates/core/tests/sweep.rs` (ignored; a survey to read): each template with every setting at
  both ends of its range, what the recipe and the G-code writer say, and timings.

**Found, not changed (the owner's call):**
- A **bell or dome wider than it's tall** (opening − thread > 2 × transition height; the
  default bell is exactly at the limit: opening 151 mm fails) can't be drawn as one arc and
  says "the profile must rise all the way". A max angle fixes the bell; the dome has no such
  setting. A step that lets a recipe say what's wrong in its own words would help here.
- The stool's foam zones go 150–300 °C in its settings; the G1 takes 160–280 °C, so the ends
  are refused when the G-code is written (rightly, but late).
- On a new device the G1's recipe origin is EasyGCode's X125 Y125 on a 1000 mm bed: the stool
  as shipped leaves the bed until the origin is set (500/500).
- Still open from before: the HUD counts primer lines as layers (the stool shows 14 for 11).

## 2026-11-09: along the surface, one layer height a turn

**Why:** sliced along the surface, each turn stepped the bead's own extent along the wall
(pitch·cos + line·sin of the lean): on a flat rim a whole line width, so each ring was laid
beside the last with nothing under it (the owner's screenshots).

**Landed:** along the surface every turn steps one pitch (the layer height) along the wall, so
each bead lies on the one before wherever the wall goes, the bead standing along the wall as a
robot arm would lay it. No overhang compensation then (it makes up for slicing a leaning wall
by height; the Compensation setting hides while slicing along the surface). Lampshade and Lamp
templates; by height nothing changes (golden tests).

**Open:** on a Cartesian printer a flat rim sliced this way pushes each ring against the last
at the same height; it's for the robot arm first.

## 2026-11-08: the lamp builder

**Why:** every vase-mode wall is a thread on both faces, so shades can screw into each other,
inside or outside, up or down; the owner wants to plan such lamps as a sandbox, without the AI
(`docs/lamp-builder.md`, decisions there).

**Landed:**
- The language: `component name … end` (with `input` and `port`) and `part name =
  component(…)`. Parts expand into one ordinary recipe (`easycad_lang::expand`): each part's
  names its own (`tube.shape`), shared ones shared. `set_values` writes a part's setting into
  its own line (adding it there when missing); `add_part` and `remove_part` (with what's
  mounted on it) are commands, in wasm as `addPart` and `removePart`.
- The engine: parts' settings carry their part, inputs show as settings (`kind: "port"`), ports
  come with their values, components with their `@ask` settings. `place(path, z, flip)` shows a
  part where it goes while it prints on the bed; placed parts whose beads come within three
  quarters of a width at the same height are a warning.
- The **Lamp** template: the shade is the lampshade's arithmetic mounted on a port (radius from
  the joint: a line plus the fit between shades, as the shaker's lid), any shape with a straight
  end, and tube, bell in-out and cone in-out. One shade prints EasyGCode's lampshade G-code (all
  four golden fixtures).
- The app: a fold-out per shade (its mount, shape and settings, a remove button), "Add shade",
  and the **Plan** view: the assembly cut through its axis, each shade as wide as its bead,
  `+` on every free port (tap: shape and up/down, with sizes scaled to where it goes), a shade
  tapped opens its settings, collisions in red. Template updates keep a project's parts.

**Open:** see `docs/lamp-builder.md` ("Still open"). No print yet of two shades screwed
together; the shaker's lid says the fit works.

## 2026-11-08: slicing along the surface, and a nozzle that follows the wall

**Why:** sliced by height, a shade that opens out flat gets its turns further and further apart
along the wall, until they don't touch; a robot arm (the next printer, `docs/roadmap.md`) would
print it along the surface with the nozzle along the wall. Both as options: everything prints
as before unless they're switched on.

**Landed:**
- `vase_revolve(…, surface: true)`: each turn steps its `layer` along the profile's own length
  instead of up it; the fields read z as the distance along the profile, and the profile may
  run flat (rings side by side at one height). Off by default; by height the arithmetic is
  untouched (golden tests unchanged).
- The lampshade's **Slicing** setting (Overhangs: height, surface). Along the surface each turn
  steps the bead's own extent along the wall: pitch·cos(lean) + line·sin(lean) (at most
  √(pitch² + line²) = 6.5 mm), squeezed by layer-height compensation as before, and an optional
  **flow where it runs flat** (blending in from 60° to 90° of lean). A test piece: the bell
  with max angle 90°, sliced both ways.
- `easycad_core::wall::up_the_wall`: at each point, the direction from the bead underneath (one
  turn earlier on a spiral, else the nearest point of the layer before) square to the travel.
  The engine sends it to the view (`paths.ups`).
- The view's **Nozzle** picker (next to Simulate): upright (a Cartesian printer's), tilted along
  the wall up to 45° (a robot arm with pellets, which need gravity), or along the wall. The
  nozzle, the bead outline under it and the beads' cross-sections stand along it. Preview only;
  remembered on the device.

**Open:** a Cartesian printer printing the flat rim along the surface hangs each ring off the
one before it (arc-overhang style); it needs a real test print with the slower flat flow.
The nozzle direction isn't part of the toolpath yet (the view works it out), so nothing can be
sent to an arm.

## 2026-11-08: the beads as the plastic actually lies

**Why:** the line-width view drew each move's *planned* bead. What comes out depends on what's
under the nozzle: the same plastic in a smaller gap squeezes out wider. The idea is from
jüngerkühn's TOOLPATHS (a "global heightfield"); the code is our own.

**Landed:**
- `easycad_core::deposit`: a heightfield over the bed (a quarter of the narrowest bead per cell,
  coarser for a big or long print; each printed part on its own empty bed). Move by move in print
  order, the gap under the nozzle sets the bead's height and its volume (planned bead × the
  move's extrusion × the printer's multiplier) its width, by the stadium bead model; the bead is
  then laid into the heightfield. The bead coming out of the nozzle and the layer's own beads at
  the nozzle's level don't count as floor (beads of one layer lie side by side; a vase's turn
  below still does). Flags: over 1.5× the planned width, nothing under it for 1.75 layers, the
  nozzle into the print; moves extruding under half a bead (a ramp running out) only flag a gap.
- The **Simulate** button (next to Line width): full-quality runs ask the engine for it; the
  beads are drawn at their simulated size, two colour modes show the simulated width and the
  squish (simulated ÷ planned), the stats line shows where a move lies, and the Print tab lists
  what it found. Remembered on the device; a printer change (multiplier) runs it again.
- What it finds in the templates (`cargo test --release -p easycad-core survey -- --ignored`):
  the stool lies as planned but for a handful of beads (the overhang drop up to 1.3×); the spiral
  disc and the thread exactly as planned; the vessel's and shaker's fillet foot squeeze 2–4×
  (their first fillet turns rise a fraction of a millimetre over the bottom's edge while still
  extruding a full layer: the squeeze they use on purpose); the lampshade has 22 moves at the rim
  whose turn steps up further than the bead is tall. 10–350 ms per template.

**Open:** the next step, the bead width following the layer below for printing (roadmap,
Later). The simulation is per cell, so a bead narrower than about four cells is coarse (prints
over a metre across).

## 2026-11-06: cleanup, from a code review

A review of `main` for dead code, duplication and complexity found little dead code; these
batches act on what it found, one commit each.

**Batch 1, a safe sweep:** `thiserror` out of `easycad-core` (only `easycad-lang` uses it);
`Plane::height_of` test-only; five exports used only in their own file (`UNIT_LABELS`,
`autoStep`, `costOf`, `TOOLS`, `validate`) no longer exported; two unused ids gone; the phase-0
UI mockup (`docs/mockup/`, superseded by the app) deleted with its links; CI runs the proxy's
tests once (its own job), not twice; `architecture.md` no longer describes the Graph view or the
old phone layout; October's dev log moved to `devlog-2026-10.md`.

**Batch 2, shared helpers:** `dom.js` has `load`/`save`/`loadJSON`/`saveJSON` for the browser's
storage (a fallback when storage is missing or refuses, never an error), in place of 24 copied
try/catch blocks in `app.js`, `chat.js`, `print.js` and `settings.js` (tested in
`settings.test.mjs`); one formatter for numbers people read (`fmtUpTo`; `agent.js` keeps its own,
locale-free, for the model). The browser tests share `scripts/test-helpers.cjs`: the web server
wait and the stand-in proxy (CORS, password, `/check`, the body decoded once).

**Batch 3, one vase ramp:** the start ramp and closing turn of `vase_spiral`, `spiralize`,
`vase_revolve` and `helix` were the same arithmetic written four times; they're now
`ramp_up`/`ramp_down` in `toolpath.rs`, and every golden G-code file is byte-identical.
`steady_start` (a steady flow through the first turn, for pellet extruders) was the lampshade's
alone; it's now `Toolpath::steady_start` and an input of all four steps (default off), and a
setting in the thread connector and the vessel (when its bottom isn't a fillet, which starts at
full flow), off by default so their golden files stay the same. The shaker starts at full flow.

**Decisions:** the kernel-probe spike is gone (its results were already in `research.md` §4.2;
its code is in the history, 8fdfc8d). `smooth` and `interpolate` fit our own cubic spline
(centripetal, curvature-continuous, periodic when closed; a tridiagonal solve) instead of
`curvo`'s NURBS, so `curvo` and `nalgebra` are gone with their 70-odd dependencies: the engine
went from 1,159,545 to 1,039,411 bytes (330 to 296 KB gzipped). No template smooths; the shapes
differ slightly from the NURBS fit (it sampled evenly in parameter, this samples per span, so
every `per_point`-th sample is a corner). The old branch `feature/issue-1` was already gone.
CI runs the smoke test on pull requests that change `web/` (or the test), Playwright 1.56.1,
with the screenshots kept as the run's artifact.

**Batch 4, smaller files (part):** the step implementations moved from `nodes.rs` (now the
table and the call machinery, 2,293 lines with its tests) into `nodes/ops.rs`; the chat's text
(Markdown, tool-call lines, thinking, money) from `chat.js` into `chat-text.js`.

**Open:** splitting `viewport.js` and `app.js`'s `wire()` waits for the playback commit on
`claude/stoic-rubin-a9kl9m` (a758abd, which changes both) to reach `main`, so the two don't
collide.

## 2026-11-03: the nozzle glides along each move

**Found:** playing back, the nozzle waited at the start of a long straight move for as long as
it takes to print, then jumped to its end.

**Landed:** the view interpolates instead: while playing, the nozzle moves along the move it's
on at the move's speed, and that move is drawn up to the nozzle (a line, or a short bead); the
print time in the stats line runs smoothly too. The G-code is unchanged.

**Why not split the G-code's long lines (looked into):**
- Size: split at 5 mm, the owner's stool goes from 92,346 lines (3.6 MB) to about 111,000
  (+21 %), at 2 mm to about 167,000 (+80 %); the lampshade +65 % / +241 %. The spiral disc and
  the thread connector are already under 1 mm a move, so nothing changes there.
- Pressure advance and motion: nothing gained. Klipper's look-ahead runs straight through
  points on a straight line without slowing, and pressure advance follows the extruder's
  *velocity changes*; pieces of one line at one speed and one E per mm have none. More moves
  only add planner and file load (and on a slow host, the risk of the queue running dry).
- Splitting helps only where something changes *along* the line (a bead that blends, a speed
  that ramps): recipes already add those points where needed (`resample`, `bead` with fields).

## 2026-11-02 (evening): play the print, fit the view, the bead under the nozzle

**Landed:**
- **Play:** ▶ next to the progress slider plays the print back like a video, the end handle
  moving at a chosen multiple of the real print time (10× to 5000×, remembered): each move at
  its own speed, travels at the printer's travel speed. The stats line shows the print time
  reached of the total. Dragging or scrolling the slider stops it.
- **Fit** (with the view buttons, a double-click, or a double-tap on a phone): frames what's
  shown of the toolpath (between the progress handles, the parts not hidden) and orbits round
  its centre, keeping the angle: Rhino's zoom extents.
- **The bead under the nozzle** is outlined in magenta: the cross-section of the move it's
  laying (width × layer height), drawn over everything.
- **One colour scale:** height uses the same blue-to-red scale as speed, flow and the bead sizes
  (it was the accent's blue to green). The colour mode is remembered per device, which is why
  a phone left on height looked different from a computer on speed.

## 2026-11-02 (later): no mix of two deploys

**Found:** right after a deploy the app failed to start ("null is not an object … scrubInfo"):
the page had the new index.html but an old app.js. GitHub Pages serves every file under the
same URL from one deploy to the next, and its CDN can hand out old and new files together for
a few minutes.

**Landed:** CI stamps the commit on every URL that loads code (`scripts/stamp-version.py`: the
stylesheet, app.js, every module import, the worker, the wasm), so each deploy's files are new
URLs and a page only ever gets one deploy's. Offline, the service worker serves any version it
has. The sources stay unstamped.

## 2026-11-02: print progress from both ends, colour ranges by hand, settings that apply

**Landed:**
- **Print progress from both ends:** the slider under the 3D view has a start and an end
  handle. Moving the start on hides what's printed before (the top layers alone, no nozzle);
  moving the end back shows the nozzle there. Scrolling moves the handle nearer the pointer.
  The nozzle's height, speed, flow and bead show in the stats line, so the slider keeps its
  length. The first scroll no longer stalls (the nozzle is compiled with the scene).
- **Colour ranges by hand:** the legend's ends can be typed in; the full colour scale runs
  between them and anything beyond is the end colour (say, speeds inside the print, with the
  brim and primers at the extremes); ↺ goes back to automatic.
- **Settings that apply:** a `param` line can end in `when …` (`when brim`, `when shape =
  "own"`, `when shape = ["sphere", "flat sphere"]`, `when not compensation = "off"`); the
  settings window shows it only while that holds (and while the setting it names shows), and a
  group with nothing to show disappears. Every template uses it; the AI's guide asks for it.

**Open:** nothing new.

## 2026-11-01 (late): the stool's brim from the top layer

**Landed:** the bend stool can brim the top layer's outline projected down (on by default):
where the legs bend the top layer is spread furthest out, so the brim keeps clear of what
overhangs there. `offset(…, trim: true)` is EasyGCode's `offsetClosedTrim`, cutting out the loops
an outward offset makes at the spread's tight notches. Golden against the owner's own G-code
file (their settings of 1 Nov: four brim lines, 92,346 lines, every command the same).

**Found:** with four lines that brim's outermost line runs from X/Y −1.0 to 1001.0 mm, past the
G1's 1000 mm bed on every side. EasyGCode doesn't check; EasyCAD refuses to write it. The
golden test compares it through a test-only writer without the checks (`gcode::write_unchecked`)
and checks that the real one refuses it. The template ships with three lines, which fit.

**Open:** if the G1's axes really reach past 0 and 1000 (its printer.cfg `position_min` and
`position_max`), the bed check could use those limits instead.
## 2026-11-01 (late): nothing over the 3D view

**Why:** on a phone the view's controls (show, colour, line width, views, parts), the legend,
the print-progress slider and the stats covered most of the model.

**Landed:** the view has its own area with nothing on it (only the "engine failed" message);
its controls sit in a strip pinned under it, so they stay with the view while the panels scroll:
one row of controls that slides sideways on a narrow screen (its end fades out) instead of
wrapping, the print-progress slider and its info, and the legend on one line beside the stats.
On a phone the view's area is a little taller (52 % of the screen, at least 260 px) so the model
keeps its size. Same element ids, so the app's code is unchanged; the browser tests pass.

**Open:** nothing.

## 2026-11-01 (night): toolpath colours, print progress, round beads

**Landed:**
- **Colour by** (in the view bar, remembered on the device): height, speed (mm/s, as the G-code
  writes it), volumetric flow (mm³/s), line width or layer height, with a legend of the range
  over the printing moves (travels grey, and left out of the range). Lines and beads alike. The
  engine now sends each move's speed and flow with its width and height.
- **Print progress:** a slider under the 3D view scrubs through the toolpath in print order, a
  step per move (drag it, or scroll over it; Shift for bigger steps), with a generic nozzle where
  it's got to and that move's height, speed, flow and bead. Lines and beads alike; the AI's
  snapshot always sees the whole print.
- **Round beads:** the line-width view sweeps a round bead along the path, like a nozzle laying
  plastic: each move keeps its own bead; at a sharp corner (over 25°) the section pivots round
  the corner point on the outside while the inside stays at the corner's mitre, so a staircase
  seam reads as rounded steps instead of sawtooth notches; each run ends in a round cap.
- The robot-arm printing idea is written down in the roadmap (Later).

**Open:** the HUD still counts primer lines as layers; the progress slider steps by points, not
by layers.

## 2026-11-01 (evening): the bend stool with legs, matching EasyGCode

**Why:** the template only built the round seat; the owner's stool (their EasyGCode settings of
21 Jul) has three legs, a bend zone that spreads layer by layer with an overhang drop, a domed
seat, a brim and foaming.

**Landed:**
- `leg_rings`: concentric rings whose outer rings run out along legs as filleted hairpins with
  round tip caps, staircase or zipper seam, and the bend-zone spread (each point's spread kept
  in z, so a recipe sinks it with a field). A port of EasyGCode's `stoolLoop`/`discLoops`: its
  rings agree with EasyGCode's to 1e-13 mm at every spread. Reusable for anything round with arms.
- `join(…, link: true)`: each step out to the next ring printed with that ring's bead, all one
  layer (each ring of a domed layer is its own toolpath at its own height).
- `offset(…, mitre: false)`: EasyGCode's per-point brim offset.
- The template is the whole stool and ships as the owner's. Golden: their stool (88,221 lines,
  every command identical), legs with a zipper seam, a domed seat with a brim, and the four
  seat-only cases from before. The smoke test downloads it from the app with their G1 settings.
- The owner's file run through EasyGCode as of 21 Jul and as of today: the same G-code apart
  from the brim (their file's `bs_brimLines` key was renamed since, so today's EasyGCode ignores
  it and brims 4 lines instead of 3) and the final lift (now 50 mm above the print).

**Open:** the printer's own settings (zones 195 °C, bed 65 °C, the stool centred at 500/500,
travel 50 mm/s) are the Print tab's, not the recipe's: set once on the device. EasyGCode's brim
from the top layer's outline isn't built. The HUD counts the primer lines as layers.

## 2026-11-01 (later): lighter CI, and why the deploy failed

**Found:** the first deploy from `main` failed in a second, before any step ran: the
`github-pages` environment only allowed the old default branch to deploy. That's a repo setting
(Settings → Environments → github-pages → Deployment branches), not something in the code.
The pull request's run also failed once on the flaky request test.

**Landed:** CI runs on pull requests and `main` only (other branch pushes run nothing; the
feature builder still starts it by hand on its branches). It runs the Rust checks and tests
(the G-code golden tests included), the notices and fixture checks, the node tests and the web
build, with the build in parallel with the tests and wasm-bindgen-cli cached. The browser tests
are out of CI and run by hand; the separate wasm `cargo check` is gone (the web build is one).

**Open:** nothing.

## 2026-11-01: streamlined for a presentation

**Landed:**
- **Settings fold out:** each `group` is a fold-out with its name and how many settings it
  holds. The first starts open, the others closed; what you open or close is remembered by
  group name on the device.
- **The Graph tab is gone,** and with it everything only it used: `web/js/graph.js`, its CSS
  and browser test, the engine's graph commands (`easycad_core::graph`), the language crate's
  outline and text edits, and the highlight drawing. Choosing a printer still writes the
  recipe's `printer` line, now through `easycad_lang::set_printer` (tested). The wasm is
  smaller and every run does a little less.
- **Templates:** only EasyGCode's projects and the shaker (lampshade first, then vessel,
  shaker, bend stool, thread connector, spiral disc). The early demo shapes and the spoon moved
  to `fixtures/recipes/`, test-only, so the engine tests and golden tests that run them stay.
- **No explanations in the app:** the paragraphs on how things work are gone from every tab
  (and the recipe's `note` lines are no longer shown in Settings; they stay in the recipe for
  the AI). Empty states are one short line. Warnings, checks and errors stay.
- **`main` deploys:** a `main` branch now holds the live app; work happens on branches merged
  into it.

**Open:** switch the repo's default branch to `main` (Settings → Branches). The docs' deeper
history still describes the Graph tab where it was built.
