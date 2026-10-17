# EasyGCode in EasyCAD: every project from general steps

*Audit of 17 Oct 2026, against EasyGCode commit `146e06e` (read only, never changed).*

The goal: everything EasyGCode does, and anything new like it, should be a **recipe of general
steps**, not a project hard-coded in the engine. This page goes through all eight EasyGCode
projects feature by feature and names the steps that build each feature. Where a step was
missing, it was added (17 Oct 2026, the list is at the end). What is still missing is listed
under each project and in [What's still missing](#whats-still-missing).

## Summary

| Project | Status | Recipe |
|---|---|---|
| Spiral disc | **G-code identical to EasyGCode** (4 cases) | `web/templates/spiral-disc.easycad` |
| Thread connector | **G-code identical to EasyGCode** (3 cases) | `web/templates/thread-connector.easycad` |
| Spoon | **G-code identical to EasyGCode** (3 cases) | `web/templates/spoon.easycad` |
| Lampshade | Buildable. Its vase mode is golden-tested on a cylinder | not written yet |
| Vessel | Buildable | not written yet |
| Container | Buildable | not written yet |
| Coat hanger | Buildable, with three small approximations (below) | not written yet |
| Bend stool | **G-code identical to EasyGCode** (4 cases, foaming included); legs buildable | `web/templates/bend-stool.easycad` (the seat) |

**"G-code identical"** means a golden test (`crates/core/src/golden_tests.rs`) runs the
template with the same settings as an EasyGCode configuration and compares the G-code command
by command with what EasyGCode itself wrote (`fixtures/easygcode/`). The start and end blocks
are compared comments included. Every command is byte-identical.

**"Buildable"** means every feature maps onto existing steps as described here. Writing the
recipe is now recipe work (an agent can do it), not engine work. Its G-code would be EasyCAD's
own, not checked line by line against EasyGCode's.

Recipes were written for four projects so far, on purpose. They prove the new steps produce
exactly EasyGCode's numbers. The others are a matter of writing recipes when they're wanted.

## Thread connector (golden)

| EasyGCode feature | EasyCAD steps |
|---|---|
| ISO 261 coarse sizes M2–M64 (diameter, pitch) | a dropdown `param size = "M8" of […]`; `index_of` + `item` look the row up |
| Line width 0 = auto 2.4 × pitch, never narrower than the pitch | `pick(line, line, 2.4 * pitch)`, `max(…, pitch)` |
| Zero-clearance fit on the 60° flanks, fit tolerance on the diameter | arithmetic: `d - (7 * sqrt(3) / 8 - 1) * pitch + fit` |
| Chord tolerance (0.01 mm) for the number of moves per turn | `circle(r, tolerance:)` (EasyGCode's rule, at least 24) |
| Seam at the back (+Y), counter-clockwise rising (right-hand) | `rotate(90)`; `helix` follows the circle's start and direction |
| Ramp turn from Z0, exactly one pitch per turn, any fraction of a turn, flat closing turn | `helix` |
| Brim rings outside the wall, outermost first, own width, layer and flow | `circle(radii)` with `series`, `toolpath` |
| Hop over the brim at twice its layer height | `join(…, clear: 2 * brim_layer)` |
| Fan on after the helix's first turn, not after the brim | `join` counts one-layer parts at the start as part of the first layer |

## Spoon (golden)

| EasyGCode feature | EasyCAD steps |
|---|---|
| Archimedean spiral: whole or fractional turns from a start radius, one line width apart, chord-tolerance sampling | `spiral(spacing: line, start:, turns:)` |
| Stick straight out along the radius through the spiral's end | `evaluate(coil, 1)`, `sample(x, …)`, `point`, `line` |
| Turned 45° onto the bed's diagonal and recentred | `rotate(45)`, `center` (both curves moved together) |
| Print from the stick's tip instead of the centre | a toggle; `flip` + `pick` |
| Stick extruded for its own bead (width, layer) without moving it | `bead(width:, layer:)` on the stick's toolpath |
| Spiral and stick as one line without a travel | `join` (parts that meet carry straight on) |
| Layers, travelling straight over at the new layer's height | `stack(…, lift: 0)` |
| Volumetric flow mode, stick feed from its own bead | `flow:` on the toolpaths |

## Spiral disc (golden since 15 Oct)

`spiral` with a field spacing (`fade`, `distance`), `interlock`, `toolpath`, `stack`.

## Lampshade

| EasyGCode feature | EasyCAD steps |
|---|---|
| E14 / E27 / custom socket, pitch = layer height | dropdown + `index_of` + `item` |
| Profile: straight throat, tangent fillet, then cone, bell, dome, sphere or flat sphere, and a lip | `line`, `arc`, `ellipse` (or `arc` + `scale`), `fillet`, `join_curves`; `revolve` |
| Vase mode with ramp and flat closing turn | `vase_spiral` (golden-tested on a cylinder against this very generator) |
| Overhang compensation, width mode: w × min(max, 1 + k(1/cos a − 1)) | `adjust(path, extrude: min(max, 1 + k * (1 / max(0.05, cos(lean(wall))) - 1)))` |
| Overhang compensation, height mode: dz = lh(1 − k(1 − cos a)), E at the full layer | `contour(wall, pitch, pace: k)` + `spiralize` + `bead(layer: pitch)` |
| Flow ramp from throat flow to shade flow across the fillet | `adjust(path, speed: lerp(1, shade / throat, smoothstep(…z…)))` (at a set flow, speed scales the flow) |
| Wide edge down or up | `mirror` / `rotate` before the toolpath |
| Brim | `offset` or `rings`, `toolpath`, `join(clear:)` |

## Vessel

| EasyGCode feature | EasyCAD steps |
|---|---|
| Base shapes: circle, ellipse, squircle, rounded rectangle, polygon and star point-up, rounded star | `circle`, `ellipse`, `superellipse`, `rectangle`, `polygon`, `star`; `rotate(90)`; `catmull_rom` (EasyGCode's spline) |
| Dense shapes simplified to a chord tolerance | `simplify` |
| Radius profile through control points (clamped Catmull-Rom) | `catmull_rom` through (h, s) points, then `sample`/`evaluate` it, or `scale` sections by a list |
| Top curves: rounded star, custom points pushed along the normals with their own Z | `evaluate` + `normal` + `move`, then `catmull_rom(closed: true)` |
| Cross-fade from the bottom shape to the top shape | `tween` |
| Inner points lifted in Z | `move(z: …field…)` |
| Bottoms: staircase, zipper, true spiral (with the centre taper) | `rings` + `reverse` + `link_rings("staircase" / "zipper" / "spiral")`; taper with `adjust(extrude: (min(distance(c), w) + w) / (2 * w))` |
| Filleted bottom | a quarter-arc profile, sections scaled per layer, `spiralize` |
| Overhang-paced Z, fitted to the height | `contour(wall, layer, pace: 1)` + `spiralize` |
| Top finish: flat or open | `vase_spiral` / `spiralize` with `flat_top: false` |
| Randomize with a seed | `random` (in EasyGCode it only sets the UI's values) |

## Container

As the vessel. The lid's radius follows from the base (`r + width + fit` in arithmetic), and
"same direction" is `flip`.

## Coat hanger

| EasyGCode feature | EasyCAD steps |
|---|---|
| Shapes, seam at ±Y, clockwise or counter-clockwise | as the vessel; `seam`, `flip` |
| Brim, mouse ears at the corners | `offset` or `rings(inset: true)`; `circle` + `move` at the corners |
| Bottom fillet with true insets and cos(angle) Z pacing | `rings(…, inset: true)`, `contour(pace:)` |
| Weave patterns: zig-zag or sine, Z angle, coverage band around the seam, patternless bottom and top layers | `wave` with an amplitude field: bands from `greater`/`less` on `atan2(y, x)` and `z` |
| Per-zone feeds (bump feed, bridge and overhang feeds) | `adjust(speed: …field…)`; overhangs from `lean` |
| Spikes placed by blue noise with relaxation (seeded) | `scatter` (the same random generator as EasyGCode), mapped onto the wall with `evaluate` and `move` |
| Staples: out, along the tip, back; own feeds; dwell at the tip | `spikes(…, out_speed:, tip_speed:, in_speed:, dwell:)` |
| Acceleration ramps after feed increases | `accelerate` |
| Fan modes: always, or only on the bumps | `fan(level: …field…)` |
| Wall hanger: gap and pocket, tangent Béziers, transition tweens | `line`, `interpolate`/`catmull_rom`, `tangent`, `join_curves`, `offset`, `tween` |

Approximations (not missing features):
- The spike's way out is straight; EasyGCode can curve it as a quarter arc ("arc way-out").
- Spikes are extruded with the wall's bead; EasyGCode can give them their own (spike bead
  override).
- Zones are fields of position. EasyGCode tags points with zones (new bridge points, hot
  overhang points) and carries the tags through its tweens, so a zone edge can sit a point
  apart from where a field puts it.

The wall hanger's SVG export is a file format, not a step (other formats come later).

## Bend stool (golden since 17 Oct)

The template is the seat without legs (EasyGCode's default), with foaming. Four cases match
EasyGCode command for command: as it ships (one layer, volumetric flow); foaming over four
layers at a set feed; foaming with a cool-down below the printer's temperatures, a zigzag exit
primer, primers at a set flow and another size and centre; three plain layers without start/end
blocks.

| EasyGCode feature | EasyCAD steps |
|---|---|
| Concentric rings, diameter snapped to whole rings | `series` of radii; `round` |
| Staircase seam: each ring stops a line short and steps out to the next | `arc(radii, start:, end:, tolerance:)` with the starts accumulated (`accumulate`, `mod`), strung together with `polyline(flatten(vertices(rings)))` |
| Zipper seams | `link_rings("zipper")` |
| Three legs as nested hairpins with fillets and tip caps | `line`, `arc`, `join_curves`, `fillet` (or `polyline` + `fillet`), nested with `offset` |
| Bend-zone spread around attractor points | `offset` or `move` by a field (`fade` of `distance`) |
| Overhang drop, dome | `move(z: …field…)` per layer; `spiralize` or `toolpath` extrude for the local layer height |
| First layer outside-in with an entrance primer line | `reverse` of the seat's `vertices`, a lead-in point along the radius, `polyline` |
| Two-move wipe between layers (diagonal to two layers above, then down) | `stack(wipe: 2 * layer)`, `join(wipe:)` |
| 15° rotation and bed recentre by the outer ring's outline | `rotate`, `center`; the offset read back with `sample(x, …)` and applied with `move` |
| Brim following the bottom or the top outline | `offset` or `rings` of either outline |
| Foaming: hotter zones from the second layer to the one before last | `temperature(up:, mid:, down:, at_end: true)` after the first layer |
| Tolerant waits (`TEMPERATURE_WAIT`, within 2 °C) on primer lines at the bed's corner | `wait_temperature(…)` and `wait_temperature(…, cool: true)` on `toolpath(line(…))` |
| Less extrusion, more speed, the same flow (`M221`, `M220`) | `extrusion_override(38)`, `speed_override(10000 / 38)` |
| Back to the printer's own temperatures, optionally cooling below them first | `temperature()` (nothing given: the printer's own), then `wait_temperature()` |
| Hops at twice the tallest print so far | `join([...], clear: 2 * layer)`, then `clear: 2 * (layers - 1) * layer` |
| Zigzag exit primer | `polyline` of points from `series`, `mod`, `greater`, `less`, `floor` |

Not exact: the spread's fillets collapsing into true corners, as EasyGCode builds them per piece.
A general offset by a field gets the same idea, not the same numbers.

## What's still missing

| Gap | Why it isn't built yet | What it takes |
|---|---|---|
| Spike way-out as an arc, and spike bead override | Small refinements of `spikes` | Two more inputs on `spikes` |
| Zone tags carried through tweens | Fields cover the projects as they are | Per-point attributes on curves, if a project needs exact zone edges |
| SVG export | An export format, not a step | Later, with 3MF and STEP |

These gaps are the first candidates for the automatic feature requests in
[feature-requests.md](feature-requests.md): the in-app agent files one when a user asks for
something the steps can't do, and a Claude Code session builds the step.

## Steps added for this audit (17 Oct 2026)

- **Lists and logic:** `pick`, `index_of`, `sum`, `accumulate`, `greater`, `less`.
- **Curves:**
  - `circle(r, tolerance)`, `superellipse`, `catmull_rom`, `fillet`, `simplify`;
  - `subcurve`, `join_curves`, `tangent`, `normal`, `closest`, `centroid`;
  - `spiral(start, turns)`, `offset` by a field;
  - ring fills: `rings`, `link_rings`.
- **Fields and points:** `lean`, `distance(point)`, `scatter`.
- **Transforms:** `center`.
- **Surfaces:** `contour(pace)`.
- **Toolpaths:**
  - new steps: `helix`, `bead`, `adjust`, `accelerate`, `fan`, `dwell`, `spikes`;
  - `vase_spiral` and `spiralize` gained `flat_top`;
  - `stack` gained `lift`, with EasyGCode's clearance hop;
  - `join` gained `clear`, continuous joins and the first-layer rule.

Machine commands that are new in EasyCAD's G-code:
- `M106 S…` / `M107` from `fan`;
- `G4 P…` from `dwell`;
- `F` values scaled by `adjust` and `accelerate`;
- extrusion above one bead from `adjust`.

They are checked against a hand-checked fixture (`fixtures/hand-checked/machine-events.gcode`,
worked out in exact arithmetic by `scripts/hand-checked-fixtures.py`).

## Steps added for the bend stool and foaming (17 Oct 2026)

- **Curves:** `arc(…, tolerance:)` (EasyGCode's chord rule, at least 12 segments), `vertices`.
- **Toolpaths:**
  - `stack(wipe:)` and `join(wipe:)`: EasyGCode's wipe between layers;
  - `stack` takes 0 copies, and `join` keeps an empty part's machine events;
  - `join`'s first-layer rule now goes by height: a part that starts no higher than the first
    layer (a brim, a primer) belongs to it, the next layer up doesn't.
- **Machine events:** `temperature`, `wait_temperature`, `speed_override`,
  `extrusion_override`. Each is placed before a toolpath prints (after the travel to it) or
  after it.

Their G-code:
- the pellet printer (EasyGCode's foaming, golden-tested with the bend stool):
  `_GINGER_EXTRUDER_SET_UP/MID/DOWN S=…`, `TEMPERATURE_WAIT SENSOR=extruder2 MINIMUM=…` (hot)
  or one `MAXIMUM=…` per zone (cool), `M220 S…`, `M221 S…`;
- the filament printer: `M104 S…`, `M109 S…` (it can wait for heating only), `M220`, `M221`.
  These are checked against a hand-checked fixture (`fixtures/hand-checked/filament-machine.gcode`).

The statistics count the overrides: time at the override's speed, material at its extrusion.

One difference from EasyGCode remains: its cool-down never goes above a zone's own temperature
(it caps the cool-down at each zone's normal one). A recipe doesn't know the printer's
temperatures, so the template asks for a cool-down below them.
