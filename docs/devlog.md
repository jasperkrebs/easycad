# Dev log

Newest first. Each entry: what landed, numbers, what's still open. Keep it short; it's how the
next session (human or agent) picks up.

October 2026 (from the first commit to 29 Oct) is in [devlog-2026-10.md](devlog-2026-10.md).

## 2026-11-06: cleanup, from a code review

A review of `main` for dead code, duplication and complexity found little dead code; these
batches act on what it found, one commit each.

**Batch 1, a safe sweep:** `thiserror` out of `easycad-core` (only `easycad-lang` uses it);
`Plane::height_of` test-only; five exports used only in their own file (`UNIT_LABELS`,
`autoStep`, `costOf`, `TOOLS`, `validate`) no longer exported; two unused ids gone; the phase-0
UI mockup (`docs/mockup/`, superseded by the app) deleted with its links; CI runs the proxy's
tests once (its own job), not twice; `architecture.md` no longer describes the Graph view or the
old phone layout; October's dev log moved to `devlog-2026-10.md`.

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
