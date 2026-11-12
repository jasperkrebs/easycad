# Dev log

Newest first. Each entry: what landed, numbers, what's still open. Keep it short; it's how the
next session (human or agent) picks up.

October 2026 (from the first commit to 29 Oct) is in [devlog-2026-10.md](devlog-2026-10.md).

## 2026-11-13 (later): the shade at its own layer height and line width

**Why:** the owner: the thread needs the socket's pitch, but the shade, for looks, may want
other layers; as the salt shaker's lid eases from its disc's bead into the skirt's through the
fillet.

**What landed:** `shade_layer` and `shade_line` (Print group, 0: the pitch and the line) in
the lampshade and the lamp. The throat prints at the pitch and line, the shade at its own,
through the fillet an S-curve of height (`smoothstep`) between them, at once with no fillet;
read by the profile's height whatever the way up (wide edge down, closed) and along the
surface. In the lamp a straight end is a thread too: through the end's fillet it eases back to
the pitch; a tube is all thread; a closed end's disc takes the shade's layer (its "0") and
line. The lampshade's lip keeps the shade's (it's no thread). Overhang compensation on the
local values: `rise = layer × squeeze`, the bead the full local layer; `vase_revolve`'s
`bead_layer` is now a number or a field read once per turn (a constant is byte-identical:
every golden test unchanged). Numbers (cone at 44°, 30 mm fillet, pitch 2.5 → 1.5, line
6 → 8): turns below z 20 rise 2.5 at 6 mm, then 2.49/6.02, 2.40/6.17, 2.25/6.41 … 1.11/8.00,
from z 41 on 1.081 (1.5 × cos 44°) at 8 mm; at most 0.29 mm of line per turn. Tests:
`a_shade_layer_as_the_threads_changes_nothing`, `the_shade_prints_at_its_own_layer_height_and_line_width`,
`a_lamp_shade_eases_back_to_the_thread_at_its_straight_end`, the closed end's disc test with
shade layers, every lampshade shape with a shade layer.

**Still open:** the blend runs over the fillet only (not the flow ramp's own height); a bell or
dome, which leaves the throat straight up with no fillet, changes at once at the throat's top.
The settings are the project's, not each shade's.
## 2026-11-13: a brim is a print aid, not in the Plan

**Why:** with the brim on, the Plan drew its rings round the shade's foot (it looked odd) and
they took the throat's outside, so no shade could be added there, and a shade screwed on there
counted as running into it. The brim is only there to print safely.

**What landed:** a new step `aid(path)` marks a toolpath as a print aid (a flag on its
settings): it prints and shows in 3D as before, the G-code is unchanged (golden tests green,
EasyGCode's lampshade with a brim included), but the Plan leaves it out (the engine sends
`paths.aids`) and the collision check ignores it. The lampshade and the Lamp's brims are
`aid(…)`. Tests: `a_brim_is_a_print_aid_that_nothing_runs_into` (a tube round the throat over
the brim: fine marked, a collision unmarked), and the Plan's free faces with and without the
marks (`settings.test.mjs`).

**Not a change:** several shades on one straight section already worked (the Plan offers the
next free stretch along it once one sits there).

## 2026-11-12 (night): the single lampshade gets the lamp's shapes and the closed end

**Why:** the owner wanted a closed lamp from the single lampshade too, with every shape the
lamp's shades have.

**What landed:** `lampshade.easycad` now runs the lamp's shade arithmetic (generated from the
component, its own setting names kept: `throat`, `lip`, `lip_length`, `lip_fillet`): shapes bell
in-out, cone in-out and tube; a lip on any shape (mirrored fillet for shapes opening out); the
closed end (disc with its own layer height, its own fillet, printed disc down, spiral ending on
the wall's first point); its marks in the Plan, drawn as it prints. Its four EasyGCode golden
cases are unchanged. The closed end's fillet now meets a curved wall tangent (the meeting
point's lean, found in two rounds, not the end's), in the lamp as well. Tests: every shape ×
(plain, wide down with lip and brim, lip without compensation, closed), no warnings; the
disc-into-wall test runs on both templates.

**Still open:** the lampshade and the lamp's shade are two copies of the same steps; a change to
one should go to the other (or the lampshade could become a one-part lamp).

## 2026-11-12 (evening): a closed end's disc runs straight into the wall

**Why:** the owner's closed cone (opening 195 mm, closed fillet 18 mm) stepped 1.4 mm outward
where the disc met the wall. Not a turn out of line (both start at 90°): the disc's spiral ended
on its outline (63.0 mm), while the wall's first turn runs where the fillet is a first rise up
(64.4 mm; the fillet leaves the disc flat, so a fraction of a millimetre up is already that far
out).

**What landed:** the disc is now an exact spiral (`spiral(turns:)`, a line a turn from the
middle) ending at the wall's first point, worked out in the recipe as `vase_revolve` works it
(the rise there, by height or along the surface), and turned (`rotate`) so it ends at the seam.
The nozzle runs on from the disc into the wall with no step. Test
`a_closed_end_runs_from_its_disc_into_the_wall_without_a_step` (height with each compensation,
and along the surface; fails on the old disc at the 63.0 → 64.4 mm step).

**Still open:** the disc's radius copies `vase_revolve`'s first-turn arithmetic in the recipe
(as the brim does); a change there needs the recipe to follow (the test would catch it).

## 2026-11-12 (later still): a straight start of 122–126 mm "went down"

**Why:** the owner's lamp failed with "the profile must rise all the way" for a straight start
of 122 to 126 mm, and worked for any other length. Where the straight start meets its fillet
the profile has two points at the same height, the second worked out the long way round
(`z_start + (throat − z_start)`): for some lengths that rounds to 124.99999999999999 after 125,
and `vase_revolve`'s check was strict.

**Landed:** `vase_revolve` takes a step down under a nanometre (`PROFILE_ROUNDING`) as level and
lifts it to the height before; a real step down is still an error. Profiles that rise are
unchanged (golden tests). Regression tests: the owner's shade at 122–126 mm (failed before), and
a profile with a 1.4e-14 mm dip.

## 2026-11-12 (later): a closed shade compensates its overhangs as every shade does

**Why:** the closed end's wall was printed with the shaker's steps (flat layers, `contour` +
`spiralize`), which don't do the lamp's overhang compensation; the owner wants the fillet into
the disc compensated like everything else.

**Landed:** a closed shade's print profile is the closed one (from the disc's edge up the
fillet and the body to the thread, its lean from 90° at the disc down to the wall's), printed by
the same `vase_revolve` as every shade, so compensation, slicing along the surface, the flow
ramp and the exact pitch at the thread all apply; the disc first, the step onto the wall printed
(`join(link: true)`: one travel, to the disc's start). Tested: the fillet's first turn rises
under 1 mm (layer height compensation), the thread's turns a pitch apart.

## 2026-11-12: a closed end for a shade; the prints overview

**Why:** the owner wanted a lamp's last shade closed at its far end (a flat disc through a
fillet into the wall, printed disc first), and a view of every print laid out on its own bed,
like a slicer's plates, where the way up and the G-code are chosen per part.

**Landed:**
- **Closed end** on the lamp's shade (`closed`, `closed_fillet`, `closed_layer`): the shaker's
  way, from general steps (`rings` + `link_rings` for the disc, `contour` without pace +
  `spiralize(ramp: false)` for the wall, `join`), so the thread keeps its pitch. The fillet is a
  tangent arc from the floor to the wall's lean at the end (kept inside the disc). Placed like a
  shade printed wide edge down, the disc's layer further. Tested: disc first on the bed, a pitch
  a turn to the thread at 23 mm, shown with the disc at the top, no end ports.
- **Prints** view (`viewport.setLayout`): each part's toolpath with its placement undone (the
  engine sends each toolpath's `views`), the parts side by side on bed outlines; a card per part
  with Flip (`wide_down`; "Disc down" for a closed one) and its G-code.
- A test's "no part left" looked for "part " anywhere and found "apart (" in a comment: it
  looks at lines now.

**Open:** the overview's colours are still by the assembly's height. (A closed shade's wall
without compensation: fixed the same day.)

## 2026-11-11: the lamp taken apart (an animation and the exploded view); the Plan on a phone

**Why:** on the owner's phone a mark tapped in the Plan didn't open its setting, the plan
didn't zoom, and dragging a shade felt wrong; and the screw preview, one shade at a time from
its bar, wanted to be an animation like the print's, all the shades, ending in an exploded view.

**Landed:**
- **Taps on a phone:** a press counted as a pan once it had wandered 6 px (summed up), and a
  finger wanders more: the mark's tap was swallowed. Now it's how far from where it started,
  12 px for a finger (5 for a mouse). Tested with a wobbling touch on an emulated iPhone, where
  `main` did nothing.
- **Zoom on a phone:** Safari's gesture events zoom the plan too (when two-finger pointer
  events don't), and `+`/`−` buttons at the plan's top right.
- **Dragging dropped**; the screw preview's panel and the bar's screw button too.
- **The assembly row** (with the print's, and in the Plan): play, a slider, **Exploded**. A step
  per shade on another (furthest out first): turned off its thread, one pitch a turn, then on by
  a gap as long as the thread; the shades already off ride along without turning (only the
  shade coming off turns). `assemblySteps` and
  `assemblyMotion` in `web/js/parts.js`, node-tested; the Plan draws the shades moved too.

- **PNG is what's shown:** in the Plan, the plan (`capturePlan`: the parts where they are,
  taken apart too, their names and the axis, without the controls, the marks or a selection's
  halo); in 3D, the 3D view as it is (taken apart too).

**Open:** the steps take equal time, a long thread spins faster; no collision check while
moving.

## 2026-11-10: a reload right after a change kept the change; the smoke test's settling

**Why:** CI's smoke test failed on PR #20 ("offered again after updating"): the project is
saved on the device 300 ms after the last change, and the test reloads at once after a
template update, so the update was lost and offered again. The same happened to anyone
reloading or closing the tab within that moment. Separately, the plan's 40 px check sometimes
measured the buttons of a drawing just replaced (0 px), a race in the test.

**Landed:** the project is saved at once when the page goes (`pagehide`) if a save is waiting.
The smoke test measures the add buttons once the plan has settled.

## 2026-11-10: slide a shade along its thread; screw it off in 3D

**Why:** the owner wanted to plan by hand: move a shade along the threaded section it sits on,
and see the threading itself, turning a shade on and off.

**Landed:**
- **Drag** the selected shade in the Plan: along the straight section it's screwed onto, the
  shades mounted on it with it, a pill reading how far along; letting go writes its `along`
  (`threadOf` and `withMounted` in `web/js/parts.js`, node-tested). On the socket it doesn't
  slide; where its straight start fills the section there's no room to (the tube on the
  sphere's 20 mm end).
- **Screw** (the bar's helix button): the 3D view with a slider of turns; the shade and what's
  on it turn about the axis (a right-hand thread: counter-clockwise seen from where it advances)
  and move out one pitch a turn, to "off" (its straight start plus how far along it sat). The
  view turns and lifts the parts' objects (`viewport.setPartMotion`); nothing is evaluated.
- The smoke test drags the shade inside the tube and screws it 4 turns (10 mm).

**Open:** the screw preview doesn't check the turning shade against the others (the collision
check is the engine's, for placed parts at rest).

## 2026-11-10: the Plan as the place to work, recipes that explain themselves, the G1's origin

**Landed:**
- **The Plan (lamp):** every free port of the selected shade has add buttons, a `+` with an
  arrow up or down, on the side of the wall the new shade screws onto (outside right, inside
  left). One tap adds a cone sized for where it goes, selected, its own buttons showing; the
  add dialog is gone. A tapped shade is selected (a halo, the others dimmed) with a bar at the
  Plan's foot: its name (opens its settings), shape, which way it goes (tap to flip), settings,
  remove, its on/off settings. Wheel or pinch zooms, drag pans, Fit re-fits; drawn in screen
  pixels so lines and buttons keep their size. Details in `docs/lamp-builder.md`.
- **Found on the way:** moving any slider of a lamp's shade wrote every one of the shade's
  settings into its part line; only the setting moved is written now.
- **`require(value, test, message)`:** a recipe says what's wrong in its own words. It passes
  its value through while the test holds; otherwise the line's error is exactly the message
  (`warn: true`: a warning) and the steps reading it are skipped, so it's the only message. The
  lampshade and the lamp's shade use it for a bell (no max angle) or dome wider than it's tall
  (it used to say "the profile must rise all the way"). The AI's guide asks for it.
- **The G1's origin is its bed's centre,** from the printer: 500/500, following the bed when
  its size is changed (an origin moved by hand stays). A saved 125/125 becomes the centre. The
  P1P keeps EasyGCode's 125/125. The smoke test sets 125/125 where it compares with
  EasyGCode's fixtures.
- **Research:** `docs/grasshopper.md`: a toolpath into Rhino, the engine inside a script
  component, definitions from recipes, reading Grasshopper files, building without the AI.
  Nothing built, nothing tried in a Rhino.

**Open:** a selected tube shows four pairs of add buttons, crowded on a phone until zoomed; the
free-port check looks at a new shade's straight start only (a bad spot shows red after the
add). Proposed, not built: tap a mark and add a step to it (a fillet), picking a shade in the
3D view, drag handles in the Plan, renaming a part, undo for every edit, the Plan as a PNG,
marks for the other templates.

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
