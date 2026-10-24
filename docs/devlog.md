# Dev log

Newest first. Each entry: what landed, numbers, what's still open. Keep it short; it's how the
next session (human or agent) picks up.

## 2026-10-24: CI pinned to Ubuntu 24.04

**Why:** GitHub moved `ubuntu-latest` to Ubuntu 26.04, and Playwright 1.56.1 can't install
Chromium there ("Playwright does not support chromium on ubuntu26.04-x64"), so the smoke test
and the deploy after it stopped. The workflows now run on `ubuntu-24.04`. **Open:** move to
`ubuntu-latest` again once a Playwright release supports 26.04 (bump the pin in both workflows).

## 2026-10-21: decimal commas on the phone

**Why:** an iPhone in a comma-decimal language shows a comma on the number keypad, and a
`type="number"` field refused what it typed ("1,5").

**Landed:** number fields are text fields with the decimal keypad (`inputmode="decimal"`), read
by `parseNumber`: a comma or a point, a typographic minus; `"1,000.5"` is 1000.5 and `"1,2,3"` is
not a number. The decimal keypad has no minus key (neither did the old one), so a field that can
go negative (no range, or a range below 0) has a ± button beside it. Settings, list rows, the
printer's settings and the graph's fields. Unit test, and the smoke test types "2,5" and uses ±.

## 2026-10-20 (later): even waves in the lid's holes

**Why:** the owner compared the hole spiral in EasyGCode (smooth, growing waves) with EasyCAD's
(every other turn's wave very low).

**Cause:** the lid's spiral was stretched to end on the fillet *before* `interlock` solved the
wiggles. Stretching opens a 2 % sliver of gap between turns that should touch; those turns then
count as open, and the per-angle solve (each pair of neighbours must add up to the gap) has a
free alternating component that nothing pins down any more: big, small, big, small (0.38, 0.01,
0.38, 0.01 mm in the outer turns, and the centre kinked). EasyGCode's lid top stretches its
finished disc, so its waves stay smooth.

**Landed:** `spiral(…, exact:)` is gone; `interlock(…, stretch: r)` stretches after the wiggles
are solved, so the result is the unstretched wiggles scaled point for point (tested). The shaker
uses it; projects made before offer the update.

## 2026-10-20: projects catch up with their templates; the salt & pepper lid in EasyGCode

**Why:** the owner still saw the step where the spiral meets the fillet, of a size that
changed with the line width, though the layer toggles were there. The toggles are app code;
the fix was in the template's recipe, and the project on the device was the recipe from before
the fix (the app remembers a project's text). A sweep of line widths on the old and new
templates: the old container stepped out by exactly one line width (0.8 to 6 mm), the old lid
by 1.1 to 7.1 mm; the new ones widen smoothly (0.01 to 0.03 mm per point; up to 0.25 mm where
the lid's hole wiggles reach the edge).

**Landed**
- **Projects catch up with their templates:** a project remembers the template it came from;
  when the template has changed since (or an older project or a file differs from its
  template), the Settings tab offers "Update" (the current template, the project's setting
  values carried over) or "Keep mine" (remembered until the template changes again). Unit
  tests for telling a template from a project made from it whatever its sliders say
  (`web/js/templates.js`); the smoke test brings an older shaker up to date and checks its line
  width carried over.
- **EasyGCode** (a branch on the fork, for a pull request): the container lid's top can be the
  salt & pepper top, the Spiral disc tab's disc with a ring of holes, ending exactly on the
  fillet at the seam (stretched a hair, every turn alike, then turned onto the seam). With the
  plain top every tab's G-code is unchanged.

**Open**
- A template's settings that were renamed don't carry over (values go by name).

## 2026-10-19 (evening): smooth into the fillet, a panel that scrolls, parts like layers

**Why:** the owner tested the shaker. Where the flat spiral met the fillet there was a step
outward (two screenshots), not the smooth run of EasyGCode's vessel; with many settings,
scrolling them took the 3D view off screen; and the two parts shouldn't print side by side:
both on the bed's centre, each shown or hidden like a layer, each its own G-code.

**Landed**
- **The step was the recipe's, not slicing's:** the rings stopped a line inside the fillet's
  foot and a straight move went out to it. The container now rings the foot itself, so
  `link_rings(…, "spiral")` widens its last turn onto it (EasyGCode's ring fill) and ends on the
  seam, where the wall starts. The lid's spiral disc ends right on its foot with a new general
  option, `spiral(…, exact: true)`: snapped to whole turns as before, then every turn stretched
  alike (at most about half a spacing over the radius). Tested: nothing in the last turn steps
  outward, it ends on the seam, and the wall carries straight on.
- **The page no longer scrolls:** an app shell. The panels scroll on their own under pinned
  tabs; the 3D view stays in sight (beside them on a desktop, above them on a phone, 42 % of
  the height). The smoke test scrolls the settings and checks.
- **Parts like layers:** a toggle per part in the 3D view's bar (from two parts up) shows or
  hides the part's toolpath with the shown shapes only it is made from. The engine says which
  part each drawn piece belongs to (`Drawing::mesh_parts`, `curve_parts`, `path_parts`): the
  print's toolpath, and every shown step that only that print is made from. Shared ones stay.
  The agent's `look` still sees every part.
- **The shaker:** both parts on the bed's centre; "Lid prints beside the container" is gone.

**Numbers:** 157 core tests (`every_drawn_piece_knows_its_part`, the exact spiral, the
shaker's smooth ends and centring), 24 language tests.

**Open**
- The toggles are for the view only; the Print tab still lists every part (each file is
  separate anyway).

## 2026-10-19 (later): the spiral discs' own bead, easing into the walls'

**Why:** the owner wanted the shaker's spiral discs (the container's bottom, the lid's top) to
have their own layer height and line width (0: the walls'), easing into the walls' through
each fillet: very gradually, point by point (within a turn too), for the toolpath itself, the
extrusion and, through the flow, the speed.

**Landed**
- **A bead per move:** a toolpath's moves can each have their own line width (as they already
  could their own layer height); 0 is the part's. Joins, stacks, moves, waves, `accelerate`
  and `spikes` keep it, and the G-code's extrusion and feed follow it (they go by each move's
  bead area).
- **`bead(path, width:, layer:)` takes fields**, read at the middle of every move: a bead that
  blends from one size into another. Plain numbers work as before.
- **`contour(surface, layer, pace)` takes a field for the layer** (read on the seam at each
  height): each layer as tall as the field halfway up it, the steps fitted to end at the top,
  paced on overhangs as before. A number gives exactly the layers it did.
- **The shaker:** "Spiral disc layer height" and "Spiral disc line width" (0: as the walls).
  The discs use them (the container's rings and the lid's spiral, holes and interlock follow
  the disc's line); up each fillet a `smoothstep` of height eases the layer spacing and every
  move's bead from the disc's to the walls'. With both at 0 the G-code is byte for byte what
  it was. With 0.3 mm × 3 mm discs under 0.6 × 2 mm walls: the disc at 0.3 mm, rings 3 mm
  apart, E per mm easing 0.881 → 1.123 mm³ and F 1363 → 1069 mm/min up the fillet, no move
  more than 0.2 % off the one before.

**Numbers:** 154 core tests (2 new, the shaker's extended), 24 language tests.

**Open**
- The G-code header still lists each part's own bead; a part whose moves blend has no single
  bead to list.
- The area of a bead blended in width and height separately can overshoot both ends a little
  (0.4 % in the example); a blend of the area itself would be another option.

## 2026-10-19: the salt & pepper shaker; several parts, list settings; inside surfaces

**Why:** the owner described a project as a test of the steps: a salt & pepper container and a
lid that threads onto it (EasyGCode's container), the lid's holes from the spiral disc, a body
shaped by radii at heights, a smooth way into the neck, a fit, and separate extrusion for the
spiral discs. Also: open surfaces showed no inside in the 3D view; and two answers about the G1.

**Assessment:** every geometric piece was already a step (the spiral disc and `interlock`, ring
spirals, the vessel's fillet, `contour(pace)` + `spiralize` + `bead`, `adjust`, `join`). Three
things were missing, all general:
- **Several parts per project:** every `print` line is a part (`print base`, `print lid`),
  named after its step, with its own G-code file (`<project>-<part>.gcode`, its title in the
  header), estimate and checks in the Print tab, and a line in what the agent sees. They share
  the settings and the printer (the first `print`'s; another printer on a later part is a
  warning). Before, only the first `print` counted.
- **List settings:** `param body = [[0, 33], [0.45, 38], [1, 32]] "…"`: numbers or rows of
  numbers, a table in the settings window (add and remove rows), JSON in the Graph tab, a whole
  list through the agent's `set_settings`; written back into the recipe like any value.
- **`graph(…, smooth: true)`:** a monotone cubic through the points (never past them), level
  at both ends; and `graph` takes its points in any order (it sorts them).

**Landed**
- **The salt & pepper shaker template** (`web/templates/salt-shaker.easycad`): the container
  (a ring spiral from the centre out, a quarter-circle fillet, the body through the list's
  radii, a neck as tall as the lid's skirt) and the lid (EasyGCode's spiral disc with its ring
  of holes, a fillet, the skirt, one line width outside the neck plus the fit), each one
  continuous line with layers closer on the fillet's overhang; both spirals turn the same way,
  so the skirt threads onto the neck. Spiral disc and wall extrusion apart. The lid prints
  beside the container, or on the same spot. Tested in `blocks_tests` (continuity, radii,
  thread pitch, pacing, extrusions, a straight body) and through the app (smoke test).
- **The 3D view shows a surface's inside**, in a shade blended toward the background.
- **The owner's answers:** the G1's vacuum feeder isn't used (pellets go in by hand), so the
  start block not switching it on doesn't matter now; the nozzle is chosen per project (mostly
  the 5 mm), so the printer's nozzle setting from the day before is gone again (the agent keeps
  Ginger's nozzle table).

**Numbers:** 152 core tests, 24 language tests, Node tests for the printers (7) and for lists
and parts (2, new); wasm 345 KB gzipped.

**Open**
- The shaker isn't golden (EasyGCode's container differs: no holes, a scale profile, a fillet
  sampled point by point).
- A drawn editor for list settings (drag the points of a profile), on top of the same list.
- With the default holes, a 6 mm line on the 5 mm nozzle needs a wider fade (`interlock` says
  so); per-nozzle defaults could come with presets.

## 2026-10-18: the pellet printer is the Ginger Additive G1

**Why:** the owner's pellet printer is a Ginger Additive G1; EasyCAD treated it as a generic
"Klipper pellet" printer. It should work with a generic Klipper printer in general, but be made
for the G1 (wiki.gingeradditive.com). The owner will add its printer.cfg later.

**Landed** (sources and details: [printers.md](printers.md))
- **The Ginger G1** replaces "Klipper pellet" in the Print tab, the chat and the templates
  (`printer "Ginger G1"`; older projects' `"Klipper pellet"` still opens it). Its bed is
  1000 × 1000 × 1000 mm (a saved 400 mm bed from before is replaced); the recipe origin stays
  at EasyGCode's X125 Y125.
- **The nozzle in the machine** (1, 1.8, 3, 5 or 8 mm; 1.8 to start), and **material presets**
  from Ginger Slicer's shipped profiles (zones, bed, max flow), in the Print tab and through
  `set_printer`'s new `material`.
- **The machine's ranges:** zones 160–280 °C, bed up to 110 °C, travel up to 300 mm/s; saved
  values outside them are clamped on load.
- **The engine refuses heater settings the G1 refuses** (`gcode::Pellet::max_temp`,
  `min_extrude_temp`, `max_bed`, from printer.cfg via the wiki): Klipper would stop such a print
  midway. Machine events included, cooling below the extrusion minimum too. Golden output is
  unchanged.
- **G1 checks in the Print tab:**
  - the flow ceiling (200, or the nozzle's own: ~150 on the 1 mm);
  - the low-flow window (below ~150 mm³/s the barrel inlet can plug), which passes once the
    upper zone runs 10 °C or more below the others;
  - line width and layer height against the nozzle (layers over about half drag it);
  - pressure advance on vase-mode prints (one continuous line over several layers);
  - the part fan over ~70 % (the toolpath stats now carry `maxFanPct`).
- **The agent's instructions** carry the G1's facts (generated from the same data), so it can
  advise on nozzles, flow, zones and materials.
- **Tests:** a heater-limits test in the writer, `scripts/print.test.mjs` (ranges, presets,
  checks; in CI), the smoke test (the G1's bed, nozzle, low-flow check, the FOAM PLA preset)
  and the agent test (`material`).

**Numbers:** 149 core tests, 7 printer tests; wasm 339 KB gzipped.

**Open**
- **The feeder:** the wiki says `START_PRINT` switches the automatic pellet feeder on, and
  EasyGCode's start block doesn't call `START_PRINT`. Does the hopper refill on EasyGCode prints?
- The start block loads the saved bed mesh (`BED_MESH_PROFILE LOAD=global`): `BED_LEVEL` has to
  be kept current (the Print tab says so).
- It fixes Rotation Volume at 456 and the mixing stepper at 8000: calibrate with the extrusion
  multiplier; a mixing setting (4000/12000) isn't there yet.
- printer.cfg (and G1-Configs' `main.cfg`/`macro.cfg`) to check the limits and the macros
  against: the list is in printers.md.
- A generic Klipper printer needs its own start/end G-code (calling its `START_PRINT`): a new
  start block, so it waits for the owner's OK and a hand-checked fixture.

## 2026-10-17: the lampshade (golden) and the vessel

**Why:** the owner asked for EasyGCode's lampshade ("works with all different shades, and easy
to create different shades": it's their current project) and its vessel (the bottom rounding
into the wall through a fillet, the non-planar top).

**Landed**
- **`vase_revolve`**: vase mode around a profile of revolution, turn by turn, as EasyGCode's
  lampshade prints it (points per turn from the radius, the radius from the profile at every
  point, a ramped first turn and a flat closing turn). Rise, width and flow can be fields read
  once per turn: that's how overhang compensation and the throat-to-shade flow ramp work.
- **`graph`** (Graph Mapper: a piecewise-linear table, also as a field), **`bounds`**,
  **`graft`** (a toolpath per brim ring), **`spiralize(ramp: false)`**.
- **The lampshade template** (`web/templates/lampshade.easycad`): E27/E14/custom sockets; the
  cone, bell (capped or not), dome, sphere and flat sphere built as profiles, plus an own shade
  (`own_shade`: any profile); EasyGCode's fillet blend onto any shade (clamped when too big);
  the lip; wide edge down; width or layer-height compensation; the flow ramp; a brim. **Golden
  in 4 cases** (cone as shipped, capped bell with a brim, sphere with a lip wide edge down,
  dome on a custom thread) and through the app. The flat sphere is sampled evenly (EasyGCode
  samples it adaptively), the one shade not checked line by line.
- **The vessel template** (`web/templates/vessel.easycad`): six cross-sections, a radius
  profile, a cross-fade into a rounded star whose inner corners rise at the rim, closer layers
  on overhangs (each extruded for a full layer), and the bottom: a fillet (one continuous line
  from the bottom's centre to the rim), ring fills or open. Built from general steps, not
  golden: EasyGCode evaluates every wall point on its surface, EasyCAD blends layer outlines.
- **The G-code writer** writes `F` whenever the exact feed changes (EasyGCode's rule). Every
  earlier golden and hand-checked output is unchanged.
- **The header** lists the first 8 parts (a lampshade has one per turn).

**Numbers:** 148 core tests, wasm 338 KB gzipped.

**Open**
- The vessel's custom top and bottom curves (points pushed along the normals) and its
  continuous spiral bottom (layers alternating into the wall) aren't in the template yet.
- The lampshade's flat sphere isn't golden (adaptive sampling).

## 2026-10-17: printers from the chat; foaming; the bend stool, golden

**Why:** the owner wants to switch printers and change printer settings from the chat, and to
build things like EasyGCode's foaming in EasyCAD.

**Landed**
- **Printers from the chat:** the agent's seventh tool, `set_printer`, switches printers and
  changes the chosen printer's settings (by the Print tab's keys, checked and clamped like the
  Print tab's fields, saved on the device). The project snapshot it gets with every message
  lists the printers and every setting with its key, unit and value.
- **The recipe names its printer:** choosing a printer (in the Print tab or from the chat)
  rewrites the recipe's `printer "…"` line through a new engine command (`SetPrinter`, which
  touches only that line, or adds it after the title), so the project opens with that printer
  next time; editing the line, or undoing, switches the printer back.
- **Machine events, as general steps** (EasyGCode's foaming, from pieces):
  - `temperature(path, nozzle, up, mid, down, at_end)`: new heater temperatures, before the
    toolpath prints (after the travel to it) or after it; nothing given is the printer's own;
  - `wait_temperature(…, cool)`: waits within 2 °C, hot or cool;
  - `speed_override` and `extrusion_override`: `M220`, `M221`, in whole percent; the print
    statistics count them;
  - the pellet printer gets EasyGCode's own commands (zone macros, `TEMPERATURE_WAIT`), the
    filament printer `M104`/`M109` (it can't wait for cooling: refused with a message).
- **Also new:** `arc(…, tolerance:)` (EasyGCode's chord rule), `vertices(curve)`,
  `stack(wipe:)`/`join(wipe:)` (EasyGCode's wipe between layers), `stack` with 0 copies,
  `join` keeping an empty part's events, and `join`'s first-layer rule by height (a primer
  after the first layer belongs to it).
- **The bend stool template** (`web/templates/bend-stool.easycad`): EasyGCode's seat with
  foaming. **Golden in 4 cases**: as it ships; foaming over four layers; foaming with a
  cool-down and reheat, a zigzag primer, primers at a set flow, another size and centre; three
  plain layers without start/end blocks. Checked through the app too (smoke test).
- **G-code safety:** the pellet commands are golden-tested with the stool; the filament ones
  against a new hand-checked fixture (`fixtures/hand-checked/filament-machine.gcode`).

**Numbers:** 143 core tests (was 136), wasm 331 KB gzipped (was 323).

**Open**
- The template's cool-down isn't capped at each zone's own temperature, as EasyGCode's is (a
  recipe doesn't know the printer's temperatures).
- The stool's legs and bend spread aren't in the template (buildable, not golden).
- New printer types (other start/end blocks) still need their own golden or hand-checked
  start/end G-code before they can be offered.

## 2026-10-17: feature requests: the agent asks, Claude Code builds, the chat carries on

**Why:** the owner wants to work only in EasyCAD. When the agent can't do something, the
missing step should get built by itself and the agent should carry on, with no one reviewing
code.

**Landed** ([feature-requests.md](feature-requests.md))
- **`request_feature`**, the agent's sixth tool. It's for when no combination of steps can
  do what's asked. It writes a precise, general proposal (what's wanted, what's missing and
  why, the step, an example), then tells the user and stops. Its system prompt says so too.
- **Filing:**
  - the proxy's new `POST /feature-request` (password-protected) files it as an issue labelled
    `feature-request`, with a token that may only write this repo's issues (secret
    `FEATURE_REQUEST_TOKEN`);
  - it refuses a sixth open request and returns the open one for a duplicate;
  - without that token, the app offers GitHub's "new issue" page, filled in: one tap.
- **The app keeps track** (`web/js/requests.js`):
  - requests are listed above the message box;
  - they're checked every five minutes and on coming back to the app (GitHub's public API, no
    token);
  - when one is done, the chat carries on by itself with a message from the app;
  - a shipped step first reloads the app for the new engine: by itself when nobody is using
    it, otherwise with a button;
  - the "Carry on by itself" setting switches this off.
- **The builder** (`.github/workflows/feature-builder.yml`):
  - it runs only for the owner's requests;
  - Claude Code (`anthropics/claude-code-action`) follows `.github/feature-builder/PROMPT.md`:
    answer with existing steps, or say why not, or build the step the EasyCAD way and run
    every check;
  - **the workflow, not the AI, ships it:** CI on the branch, the merge rules (`guard.sh`:
    no protected files, no new dependencies, no fewer tests, at most 3000 lines; G-code
    writer changes wait for review), merge, CI and deploy, then a comment that GitHub emails;
  - a failure after the merge is reverted;
  - three attempts, then it asks for a person.
- **Tests:**
  - the proxy endpoint: filing, duplicates, the cap, bad input, password first, not set up;
  - `scripts/request-test.cjs` (in CI) drives the whole loop in Chromium against stand-ins for
    Claude, the proxy and GitHub: filing, shipping, carrying on, the reload, the one-tap link;
  - actionlint and shellcheck are clean on the workflow and its scripts.

**Open**
- **Needs the owner's setup** before it runs: a Claude secret for Actions, workflow write
  permissions, and optionally the proxy token (feature-requests.md, "Setup"). Not yet tried
  end to end on GitHub.
- The app carries on only while it's open, or the next time it's opened. There's no server
  that runs the agent while you're away.

## 2026-10-17: every EasyGCode project from general steps; thread and spoon golden

**Why:** the owner's goal is that every EasyGCode project (and future ones) can be made in
EasyCAD from recipes, with the steps simple enough that even a small model can build with them.

**Landed**
- **Audit** of all eight EasyGCode projects, feature by feature, against EasyCAD's steps:
  [easygcode-coverage.md](easygcode-coverage.md). Seven are fully buildable; the bend stool
  lacks only foaming (printer heater macros, left out until it can be golden-tested).
- **About 35 general steps:**
  - lists and logic: `pick`, `index_of`, `sum`, `accumulate`, `greater`, `less`;
  - curves: `circle(r, tolerance)`, `superellipse`, `catmull_rom`, `fillet`, `simplify`,
    `subcurve`, `join_curves`, `tangent`, `normal`, `closest`, `centroid`,
    `spiral(start, turns)`, `offset` by a field;
  - ring fills: `rings`, `link_rings` (staircase, zipper, spiral);
  - fields and points: `lean`, `distance(point)`, `scatter`; transforms: `center`;
    surfaces: `contour(pace)`;
  - toolpaths: `helix`, `bead`, `adjust`, `accelerate`, `fan`, `dwell`, `spikes`,
    `flat_top` on the vase steps.
- **Toolpath model** (`toolpath.rs`):
  - per-move speed factors and machine events (fan levels, dwells);
  - EasyGCode's clearance hop in `stack` (`lift`) and `join` (`clear`);
  - joins carry straight on where parts meet, and skip empty parts;
  - one-layer parts at the start belong to the first layer, so the fan comes on where
    EasyGCode turns it on.
- **Templates** "Thread connector" and "Spoon": golden tests against six new EasyGCode fixtures
  (three each, including a brim with its own layer height, a fraction of a turn, printing from
  the stick's tip with a thicker stick bead, the filament printer and bare G-code). Every
  command is byte-identical on the first run. The smoke test downloads both from the app and
  compares them too.
- **New G-code commands** (`M106`/`M107` from `fan`, `G4` from `dwell`, scaled `F`, extrusion
  above one bead) are checked against a second hand-checked fixture
  (`fixtures/hand-checked/machine-events.gcode`, exact arithmetic in Python).
- The 3D view frames wide objects on a phone too (it used only the vertical view angle).
- Engine 323 KB gzipped (of the 3 MB budget).

**Open**
- Foaming; spikes' arc way-out and bead override; zone tags carried through tweens (see the
  coverage page). Each is a feature request waiting to be filed.
- The vessel, lampshade, container, coat hanger and bend stool templates aren't written yet:
  recipe work, not engine work.

## 2026-10-15: the Graph tab, Grasshopper-style components and wires

**Why:** the owner works in Grasshopper and wants to build and extend recipes without AI,
"understandable, editable and expandable by a human", and to keep design control over what
the AI wires.

**Landed**
- **Graph tab** (`web/js/graph.js`):
  - settings as sliders on the left, each step as a component with its pipe stages inside,
    and a wire for every name an input reads; `print` and `export` are sinks;
  - components are coloured by status and show what they made;
  - laid out left to right by dependency; where you drag components is kept on the device.
- **Editing without AI:**
  - drag a wire from an output to an input;
  - add components from a searchable palette (button, double-click, or drop a wire on empty
    space);
  - type or wire any input in the inspector below the canvas;
  - add stages (`|>`), preview (`show`), rename, remove, add sliders and change their ranges,
    move sliders; Undo;
  - selecting a component draws its geometry in green in the 3D view, as in Grasshopper.
- **Every edit is a command and a text edit** (`easycad_core::graph`, on the outline and edits
  in `easycad_lang::outline`/`edit`):
  - each changes only its own piece of the recipe (comments and layout stay);
  - each is parsed again before use, so the graph can't break a recipe;
  - an input can't be given text that reaches outside it (`1) |> move(…` is refused).
- **Tests:**
  - the outline matches the parser's references in every template;
  - every kind of edit, with exact before/after text;
  - every command on every component of every template either parses or says why;
  - `scripts/graph-test.cjs` drives the real UI with a mouse (desktop) and touch (phone): it
    types an input, adds from the palette, drags a wire, previews, renames, undoes, removes,
    adds a slider and changes it. It runs in CI.
- Engine 279 KB gzipped (of the 3 MB budget).

**Open**
- Positions live on the device, not in the file (a `layout` block is planned).
- No clusters (your own steps from a group of components) and no full data trees yet.
- Wiring an input that isn't shown on the canvas (an optional one without a value) goes
  through the inspector's "wire from…" list.

## 2026-10-15: the Grasshopper-style toolbox, and vase mode through tilted layers

**Why:** the owner's test request was a cylinder whose top is cut by a tilted plane, with the
layers in between tweened and printed as one spiral, the extrusion following the layer height.
It couldn't be built: no 3D rotation, no planes, no tweens, and vase mode only for flat layers.
The fix is a general Grasshopper-like vocabulary, not one step per project.

**Landed**
- **Points and planes** as values: `point`, `vector`, `plane`, and `[x, y, z]` anywhere a point
  is asked for (a list of them is a list of points). `x`, `y`, `z` are built-in fields: each
  point's own coordinates.
- **Transforms take fields, axes and centres.** `move(c, z: 0.1 * x)` tilts a curve point by
  point; `rotate` turns about any axis through any centre (`rotate(wall, z * 0.5)` twists),
  `scale` about a centre and by fields; new `mirror` and `project_along`. Turns about the
  vertical keep the exact old numbers (the Phase 0 parity test still passes).
- **Curve and list tools** with Grasshopper's names in their descriptions: `ellipse`,
  `rectangle`, `arc`, `line`, `polyline`, `interpolate`, `tween`, `offset`, `divide`,
  `evaluate`, `length`, `flip`, `seam`, `resample`; `series`, `random` (seeded), `item`,
  `reverse`, `shift`, `flatten`, `remap`, and more maths (`asin` … `sign`). A list of lists runs
  a list step once per inner list, like a Grasshopper branch.
- **Surfaces:** `extrude`, `revolve` (about Z, prints in vase mode), `contour` (flat layers).
- **`spiralize(layers, width, flow)`**: vase mode through any stack of closed layers, tilted
  ones too. Each move is extruded for its own layer height: the gap to the turn below, averaged
  over the move. Through flat, even layers it prints exactly like `vase_spiral`, and is
  golden-tested against EasyGCode's vase mode at a set speed and at a set flow. Tilted layers
  have no EasyGCode counterpart, so they are checked against a **hand-checked fixture**
  (`fixtures/hand-checked/`). It is worked out from the geometry in exact arithmetic by
  `scripts/hand-checked-fixtures.py`, with every move annotated and one move worked by hand in
  its README. CI checks that it's up to date.
- **Templates:** "Cut cylinder" (the owner's example: circle → tilted plane → project →
  tween → spiralize, 267 layers of 0.26–0.34 mm at 8 mm³/s) and "Turned vase", geometry only
  (an interpolated profile turned around Z, saved as STL; no `print`). Both run at every corner
  of their settings in the tests.
- The language guide (agent and Copy for AI) and `docs/definition-language.md` describe the
  toolbox, with a Grasshopper → EasyCAD table. 71 steps; engine 241 KB gzipped.
- The agent's cached system prompt (guide + 71 steps + tools) grew to about 7.3k tokens: about
  $0.02 to write the cache once, then about $0.0015 per request to read it with Sonnet 5.5.

**Open**
- Still missing for many Grasshopper definitions: Brep solids and booleans, sweep/pipe, curve
  booleans, fillets, data trees with paths, clusters (user-defined steps), Graph Mapper's curve.
- `spiralize` needs the layers' points to correspond (tweens, contours and moves do). Arbitrary
  slices of a solid come with the solid steps.

## 2026-10-15: speeds as volumetric flow

**Landed**
- The toolpath steps take `flow:` (mm³/s). Each move's speed is flow ÷ its bead area and the
  G-code feed is flow · 60 ÷ area, rounded, as in EasyGCode's flow mode. Moves can also carry
  their own layer height (for non-planar layers, next), and the bead area follows it.
- Golden-tested against EasyGCode's flow mode: the spiral disc template (now `flow: 20` by
  default) and the vase spiral (`flow: 8`). Every command identical, six cases in all. The
  speed-mode cases still pass.
- All templates set flow instead of speed. The Print tab shows the flow and the nozzle speed it
  gives, and checks the flow against a new printer setting, "max flow" (P1P: 20 mm³/s).
- The agent's guide says to use flow, and each message tells it the printer's max flow.

## 2026-10-15: the agent in the app (Chat tab) and the AI proxy

**Landed**
- **Chat tab** (`web/js/chat.js`, `web/js/agent.js`): Claude builds and edits the recipe with five
  tools (`edit_recipe`, `write_recipe`, `set_settings`, `look`, `read_recipe`) that run in the
  browser on the engine. Each change runs at once and reports back as compact text; `look`
  returns one picture with four views (perspective, and front/side/top as orthographic views).
  Streaming answers, summarized thinking, an "Undo its changes" button per message, the cost of
  each answer and of the month, the chat kept on the device (IndexedDB), and "Your rules" that
  apply to every project.
- **Model:** Sonnet 5.5 at medium effort by default (Opus 5.5 and Haiku 5.5 in settings),
  adaptive thinking, refusal fallback on. The system prompt (language guide + step catalog,
  about 4k tokens) is byte-identical on every request and cached; the chat is append-only.
- **Two ways to connect:** a password for your own **AI proxy** (`agent-proxy/`, a Cloudflare
  Worker that holds the key; CI deploys it from four GitHub secrets and writes its address into
  the site's `config.json`), or a key stored on the device. Setup: `docs/api-key.md`.
- The official TypeScript SDK 0.129.0 is vendored as one minified ES module (189 KB, 50 KB
  gzipped), loaded only when the chat is used. "Copy for AI" now shares the agent's language
  guide (`web/js/language-guide.js`).
- **Tests:** `agent-proxy/worker.test.mjs` (password, origins, models, limits, streaming
  pass-through; the Claude API stubbed) and `scripts/agent-test.cjs` (the Chat tab against a
  scripted stand-in for the proxy and the API: write → look → edit → answer; checks caching
  settings, a fixed tool list, an append-only history, the picture, undo, reload, a wrong
  password). Both run in CI.

**Open**
- The owner sets up the secrets (or a key on the device); no real API call has been made from
  CI or here yet (no key in this environment). The first real chat is the real test.

## 2026-10-15: Phase 1b, real G-code, and EasyGCode's spiral disc as a recipe

**Why:** the owner wants to build custom things inside the app, both G-code projects like
EasyGCode's and plain geometry to export, without a new app build for every project. The test:
EasyGCode's spiral disc, which was a dedicated generator, rebuilt only from general steps.

**Landed**
- **G-code** (`core::gcode`) in EasyGCode's dialect for the P1P (filament, Marlin start/end) and
  the Klipper pellet printer: start and end blocks copied verbatim, E from the bead model
  (mm³ or mm of filament), numbers rounded exactly like JavaScript's `toFixed`, F only when it
  changes, the fan after the first layer. Before writing, it checks the bed, the build height
  and every extrusion amount; the end lift stops at the build height (Klipper refuses moves past
  it; EasyGCode doesn't check, so this is the one deliberate difference).
- **Golden tests against EasyGCode itself:** `scripts/easygcode-fixtures.cjs` runs EasyGCode's
  `gcode.js` headlessly and writes `fixtures/easygcode/`. **Every command identical** in all
  four cases:
  - three spiral disc cases (default pellet, filament with a multiplier and fan, no start/end
    blocks): 8,772, 9,639 and 5,133 commands;
  - `vase_spiral` on a cylinder against EasyGCode's lampshade generator with both ends the same
    size (its vase mode: ramped first turn, whole layers, flat closing turn): 7,908 commands.

  So every toolpath the templates print is covered. The browser smoke test also checks the
  G-code the app downloads for the spiral disc.
- **Fields:** numbers that vary over space. `distance(curve)`, `fade(…)` (EasyGCode's falloff),
  and arithmetic and the number steps on fields, point by point. `wave`'s amplitude can be one.
- **General toolpath steps:** `toolpath(curves, …)` prints any curves; `stack` repeats a layer
  (EasyGCode's lift-and-return layer change); `join` prints toolpaths one after another;
  `spiral` (RK4 like EasyGCode, spacing from a field) and `interlock` (the odd-bump wiggles,
  solved per angle like EasyGCode). 34 steps in all.
- **The spiral disc template** (`web/templates/spiral-disc.easycad`): ten lines, every setting
  of EasyGCode's tab, same G-code.
- **STL export:** `export shape as "x.stl"` (binary STL of surfaces and solids; the vase's is
  closed and faces outward, which a test checks). The vase and bowl templates offer one.
- **`core::files`** makes the files (G-code for `print`, the file of each `export`), so the
  app, a CLI and MCP share one path. The WebAssembly facade got `exportFile`.
- **Web app:**
  - Settings tab: an **Export** section (G-code for the chosen printer, plus the recipe's
    `export` files);
  - Print tab: the printer choice and **printer settings** (bed, where the recipe's origin sits,
    travel speed, multiplier, end lift, temperatures, fan, pellet purge and pressure advance),
    remembered per printer, with EasyGCode's defaults; checks for bed fit and height;
  - the 3D bed grid follows the printer's bed and origin;
  - the engine client answers requests by id (runs, exports, the catalog) and re-asks whatever
    a crashed worker never answered.

**Numbers** (cloud CPU, WebAssembly in Node)
- Engine WASM: 696 KB raw / **212 KB gzipped** (budget 3 MB).
- Spiral disc: **19 ms** per run (8,737 path points), G-code 28 ms (315 kB).
- Twisted vase: 25 ms per full-quality run; G-code 0.28 s (119,617 moves, 4.4 MB); STL 47 ms
  (2 MB).
- 95 Rust tests (17 language, 78 engine). Smoke test green on phone and desktop: it also saves
  an STL and checks the spiral disc's G-code against EasyGCode's.

**Open**
- **1c:** more of EasyGCode as recipes from general steps: the vessel (wall plus a solid bottom
  as a joined toolpath), the bend stool seat (`offset_rings`), the lampshade (`revolve`,
  `spiral_on`). Each golden-tested.
- Geometry for STL: `extrude`, `revolve`, `offset`, then booleans.
- No per-step caching yet; G-code for very large toolpaths takes a few tenths of a second.

## 2026-10-15: Phase 1a, recipes become files

**Landed**
- **Hosting decided:** the repo is public, and CI deploys `web/` to GitHub Pages from the
  default branch. It needs one setting from the owner (Settings → Pages → Source: GitHub
  Actions); until then the deploy job leaves a notice and stays green.
- **`easycad-lang` (new crate):** lexer, parser and syntax tree for the recipe language v0.1.
  - Errors carry line and column and read like advice ("`in` is a reserved word; pick another
    name").
  - `set_values` writes setting values back into the text and touches nothing else: comments,
    alignment and other lines stay as they were.
  - Hostile input can't crash it: nesting and expression size are capped.
- **`easycad-core` runs recipes:**
  - the evaluator runs steps in dependency order (Kahn's algorithm, no recursion). It reports
    unknown names (with "did you mean"), loops and duplicates. An error belongs to its step; steps
    that need it are skipped and everything else still runs;
  - lists replicate Grasshopper-style (longest list), units convert to mm, degrees and ratios,
    and a budget caps how much geometry one run can make;
  - 24 nodes:
    - numbers and lists: `range`, `lerp`, `count`, `min`, `max`, `clamp`, `mod`, `abs`, `sqrt`,
      `round`, `floor`, `ceil`, `sin`, `cos`, `tan`;
    - curves: `circle`, `polygon`, `star`, `smooth`;
    - transforms: `scale`, `rotate`, `move`;
    - surfaces: `loft`, `cap`;
    - toolpaths: `vase_spiral`, `wave`;
    - plus a catalog generated from the code;
  - the hard-coded Phase 0 demo is gone. The twisted vase is now a recipe and prints the same:
    path length 135,442.45 mm vs 135,442.76 before. With no twist and the top at 2×, it matches
    to every printed digit;
  - the overhang check only searches the nearby part of the layer below (7.7 → 2.2 ms).
- **Web app:**
  - the settings window is generated from the recipe: sliders with number fields, toggles,
    dropdowns, text fields, groups, labels, ranges, steps and an "assumed" badge;
  - **Recipe tab:** the text itself, with Run and Revert:
    - problems listed by line (tap one to jump there);
    - every step and what it made ("list of 46 curves");
    - the step catalog;
    - **Copy for AI**: the recipe plus a summary of the language and the catalog, to paste into
      any Claude chat until the agent lives in the app;
  - templates: twisted star vase, ribbed cylinder (a wave on the toolpath), star bowl;
  - open/save `.easycad` and save/load settings (JSON). The project is autosaved on the device;
    "Reset settings" goes back to the values as opened;
  - the engine worker restarts itself if the WebAssembly ever traps;
  - text from a recipe only reaches the page as text, never as HTML, because recipes can come
    from anywhere.

**Numbers** (cloud CPU)
- Engine WASM: 573 KB raw / **173 KB gzipped** (Phase 0: 80 KB; budget 3 MB).
- Twisted vase, full quality (46 sections, 533 layers, 119,617 path points): **14 ms** native,
  **26 ms** as WebAssembly in Node. The preview while dragging takes 7 / 12 ms.
- 68 Rust tests (17 language, 51 engine), including a random-recipe crash test, the Phase 0
  parity test and a check that every node is in `docs/definition-language.md`. The smoke test is
  green at phone and desktop sizes: it edits a setting, edits and breaks the recipe, switches
  templates and reloads.

**Open**
- ~~**Owner:** switch on GitHub Pages~~ (done: live at https://jasperkrebs.github.io/easycad/).
- ~~**1b:** G-code for the P1P and the Klipper pellet printer~~ (done, see above); a solid
  bottom and the vessel template moved to 1c.
- **1c:** formatter, presets, a project library on the device, `revolve` / `offset_rings` /
  `contours`.
- No per-step caching yet: every run recomputes everything, which is fine at these sizes.
- Language features still on paper: domains, comparisons and `if`, records, presets.

## 2026-10-14: Phase 0 walking skeleton

**Landed**
- Rust workspace:
  - edition 2024, no-panic lints (`unwrap`/`expect`/`panic` denied outside tests), `unsafe`
    forbidden;
  - `easycad-core`, the engine: twisted star vase demo → preview mesh, continuous vase-mode
    toolpath (start ramp, whole-layer snapping, flat cap, as in EasyGCode), print stats;
  - `easycad-wasm`, a thin wasm-bindgen facade.
- Web app (`web/`) in the EasyGCode look:
  - engine in a Web Worker with latest-wins scheduling;
  - three.js 0.186.1 vendored (WebGL 2), toolpath as fat lines coloured by height;
  - settings with sliders + number fields, preview while dragging, full quality on release;
  - print estimate and checks;
  - save/load settings (JSON) and settings remembered on the device;
  - PWA manifest, network-first service worker, generated icons.
- Scripts: `build-web.sh` (with size budget), `serve.sh`, `smoke-test.cjs` (headless
  Chromium, phone + desktop), `third-party-notices.py` (license check + notices).
- CI (`.github/workflows/ci.yml`): fmt, clippy, tests, wasm check, notices check, web build,
  smoke test. Pages deploy is ready but gated on the `DEPLOY_PAGES` repo variable.
- Docs: decisions recorded (roadmap). New `toolpaths-and-skills.md`, `api-key.md`, and
  research §9 (browser games).

**Numbers** (cloud CPU, headless Chromium)
- Engine WASM: 267 KB raw / **80 KB gzipped** (budget 3 MB).
- Full-quality vase (twist 180°: 120 rings × 224 samples, 533 layers, ~120k path points):
  **~15 ms** per evaluation in the worker.
- 15 Rust tests, all green. Smoke test green on phone and desktop sizes.

**Open**
- **Hosting:** the repo is private. Pages needs GitHub Pro (or a public repo, or Cloudflare
  Pages). Owner to decide; then set `DEPLOY_PAGES=true`.
- Phase 1 next: definition language v0, evaluator, settings window generated from the
  project, project files, first G-code (P1P + pellet) golden-tested against EasyGCode.
