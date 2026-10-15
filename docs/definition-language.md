# The EasyCAD definition language (v0.1)

*Status: **v0.1 is implemented** (Phase 1a, 15 Oct 2026): the parser is `crates/lang`, the
evaluator and nodes are `crates/core`. Phase 1b (15 Oct 2026) added fields (numbers that vary
over space), general toolpath steps, and files: G-code and STL (§2.1). §2 says exactly which
syntax works today and what comes later; §6 lists the steps you can use. The format was approved
on 14 Oct 2026; syntax and node names may still change, and every file starts with its version
so old files can be migrated.*

> **In plain words:** this is the *project file*. It has nothing to do with the chat commands
> like `height 200`. When you save a project, this is what gets written:
> - your settings (the values in the settings window);
> - the recipe (the steps that turn them into a shape, curves, a toolpath and G-code);
> - optionally, presets and your conversation with the agent.
>
> When you open it again, EasyCAD rebuilds the same window, shape and toolpath. You never have
> to read or write this text yourself; the app and the agent do. It's simply there if you want
> to look.

An EasyCAD document (`*.easycad`) is a **Grasshopper definition written as text**:

- **parameters** at the top become sliders, toggles, dropdowns and draggable handles;
- every **named line** below is one node;
- every **name you reference** is a wire.

The same file is what you edit with sliders, what the graph view draws, and what the agent
writes.

Why a small language and not JSON, Python or JavaScript (details in
[research.md](research.md#35-research-on-ai--cad-2026)):

- **LLMs write compact functional CAD code most reliably.** OpenSCAD-style code beats CadQuery
  and JSON in the 2026 benchmarks.
- **Dataflow-only text maps 1:1 onto a node graph**, the way Dynamo's DesignScript does.
  Imperative code with loops doesn't.
- **It's safe.** There are no loops, no mutation and no file or network access, so a
  definition written by an AI can't do anything except compute geometry.

---

## 1. Five examples

### 1.1 Twisted star vase (sliders + list replication)

This is the app's first template (`web/templates/twisted-vase.easycad`), exactly as it ships:

```
easycad 0.1
project "Twisted star vase"
printer "Bambu P1P 0.4"
note "A rounded star that shrinks and twists as it rises, printed as one continuous spiral (vase mode)."

group "Shape"
param height    = 160 mm  in 40..300          "Height"
param radius    = 45 mm   in 10..150          "Tip radius"
param points    = 7       in 3..16 step 1     "Star points"
param depth     = 18 %    in 0..60            "Star depth"
param twist     = 90 deg  in -360..360        "Twist"
param top_scale = 0.8     in 0.3..2           "Top scale"

group "Print"
param layer     = 0.3 mm  in 0.08..3          "Layer height"
param line      = 0.6 mm  in 0.2..10          "Line width"
param speed     = 40      in 2..300           "Speed (mm/s)"

# One rounded star, copied every few degrees of twist: each copy scaled, turned and lifted.
outline  = star(points, outer: radius, inner: radius * (1 - depth)) |> smooth
t        = range(0, 1, count: 16 + abs(twist) / 3)
sections = outline
           |> scale(lerp(1, top_scale, t))
           |> rotate(twist * t)
           |> move(z: height * t)
wall     = loft(sections)
vase     = cap(wall)
path     = vase_spiral(wall, layer: layer, width: line, speed: speed)

show vase
print path
```

What you get in the app:
- a settings window with two groups (Shape, Print);
- six steps, each listed in the Recipe tab with what it made (`list of 46 curves`, …);
- a solid to look at and a toolpath to print, with its print estimate.

`t` is a list of numbers, so `scale`, `rotate` and `move` each run once per number and
`sections` is a list of 46 curves. Asking for "less twist at the bottom" changes one line
(`rotate(twist * t^2)`). Making it taller is a slider.

### 1.2 Attractor panel (the classic Grasshopper demo)

*Later: needs nodes from later phases (`rectangle`, `extrude`, `grid_points`, `difference`, …).*

```
easycad 0.1
# Perforated panel: hole size grows with distance from an attractor point

param width     = 200 mm in 50..400  "Panel width"
param height    = 120 mm in 50..300  "Panel height"
param thickness = 3 mm   in 1..10    "Thickness"
param cols      = 14     in 2..40    "Columns"
param rows      = 8      in 2..30    "Rows"
param attractor = point(60, 40)      "Attractor"        # shown as a draggable dot
param r_min     = 1.5 mm in 0.5..10  "Smallest hole"
param r_max     = 6 mm   in 1..20    "Largest hole"

panel  = rectangle(width, height) |> extrude(thickness)
grid   = grid_points(width, height, cols: cols, rows: rows, margin: r_max + 2 mm)
d      = distance(grid, attractor)                       # one distance per grid point
r      = remap(d, to: r_min..r_max)                      # one radius per grid point
holes  = circle(center: grid, r: r)                      # one circle per grid point
         |> extrude(thickness + 2 mm)
         |> move(z: -1 mm)
result = difference(panel, holes)                        # subtract the whole list

show result
export result as "panel.3mf"
```

No loops anywhere: `grid` is a list, so `distance`, `remap`, `circle` and `extrude` all run
once per item (Grasshopper's list matching). Dragging the attractor dot in the viewport
updates the parameter and re-evaluates, with no AI involved.

### 1.3 Box with lid (fit tolerances, the "SolidWorks" side)

*Later: needs solids (`rounded_rect`, `extrude`, `offset`, booleans).*

```
easycad 0.1
# Box with a press-fit lid

group "Box"
param inner_w   = 80 mm   in 20..250  "Inner width"
param inner_d   = 50 mm   in 20..250  "Inner depth"
param inner_h   = 40 mm   in 10..200  "Inner height"
param wall      = 2 mm    in 0.8..5   "Wall"
param corner    = 6 mm    in 0..20    "Corner radius"

group "Lid"
param lid_h     = 8 mm    in 3..30    "Lid height"
param lip       = 6 mm    in 2..15    "Lip depth"
param clearance = 0.25 mm in 0..1     "Lid clearance"   @assumed

outline = rounded_rect(inner_w + 2 * wall, inner_d + 2 * wall, r: corner)
body    = outline |> extrude(inner_h + wall)
cavity  = outline |> offset(-wall) |> extrude(inner_h) |> move(z: wall)
box     = difference(body, cavity)

top     = outline |> extrude(wall)
ring    = outline |> offset(-wall - clearance) |> extrude(lip)
hollow  = outline |> offset(-2 * wall - clearance) |> extrude(lip)
lid     = union(top, difference(ring, hollow) |> move(z: wall))
          |> move(x: inner_w + 2 * wall + 10 mm)          # print it next to the box, lip up

show box, lid
export [box, lid] as "box-with-lid.3mf"
```

`@assumed` tells you the agent guessed the clearance. The Params panel shows it with a badge
until you confirm it.

### 1.4 A texture made by the toolpath (EasyGCode heritage)

The second template (`web/templates/ribbed-cylinder.easycad`): the shape is a plain cylinder,
and the ribs exist only in the toolpath.

```
easycad 0.1
project "Ribbed cylinder"
printer "Bambu P1P 0.4"

group "Shape"
param radius = 40 mm   in 15..120              "Radius"
param height = 100 mm  in 20..250              "Height"
param ribs   = 12      in 3..40 step 1         "Ribs per turn"
param drift  = 0.02    in -0.5..0.5 step 0.01  "Rib drift per layer"
param size   = 1.5 mm  in 0..5                 "Rib size"

group "Print"
param layer  = 0.3 mm  in 0.08..3              "Layer height"
param line   = 0.8 mm  in 0.2..10              "Line width"
param speed  = 30      in 2..300               "Speed (mm/s)"

base = circle(radius)
wall = loft([base, base |> move(z: height)])
path = vase_spiral(wall, layer: layer, width: line, speed: speed)
       |> wave(amplitude: size, count: ribs + drift)

print path
```

This is EasyGCode's model: geometry *is* the toolpath. `wave` counts in waves per turn along
the spiral, so a whole number stacks the ribs straight up, a small fraction makes them drift
round slowly, and a half (`ribs + 0.5`) makes alternate layers interlock. Extrusion comes from
the bead cross-section (`(w − h)·h + π·(h/2)²`).

### 1.5 EasyGCode's spiral disc, from general steps

The fourth template (`web/templates/spiral-disc.easycad`). In EasyGCode the spiral disc is a
dedicated generator, written into the app's code. Here it's a recipe built from general steps,
and its G-code is the same as EasyGCode's, command for command (a golden test checks it):

```
group "Disc"
param outer    = 60 mm   in 5..150          "Outer radius"
param ring     = 35 mm   in 1..150          "Ring radius"
param open     = 200 %   in 100..400        "Spacing at the ring"
param band     = 0 mm    in 0..50           "Full-spacing band"
param fade_in  = 15 mm   in 0..150          "Fade inside the ring"
param fade_out = 15 mm   in 0..150          "Fade outside the ring"
param bumps    = 21      in 1..99 step 2    "Bumps per turn"
param layers   = 1       in 1..20 step 1    "Layers"

group "Print"
param layer    = 0.6 mm  in 0.1..3          "Layer height"
param line     = 2 mm    in 0.2..6          "Line width"
param speed    = 20      in 1..100          "Speed (mm/s)"

# Turns sit one line width apart, spreading to `open` around the ring.
near    = fade(distance(circle(ring)), inside: fade_in, outside: fade_out, band: band)
spacing = line * (1 + (open - 1) * near)
odd     = 2 * floor(bumps / 2) + 1             # odd, so a bump always faces the next turn's dip
disc    = spiral(outer: outer, spacing: spacing, min_samples: 16 * odd)
wiggly  = interlock(disc, bumps: odd, width: line)
path    = toolpath(wiggly, width: line, layer: layer, speed: speed) |> stack(layers)

print path
```

`near` is a *field* (§3): a number at every point of the bed, 1 on the ring and fading to 0
away from it. Arithmetic with a field gives a field, so `spacing` is a field too, and `spiral`
spreads its turns wherever `spacing` is larger. Nothing in these steps knows about discs: the
same `fade`, `spiral`, `interlock`, `toolpath` and `stack` build other things.

---

## 2. Syntax

Everything below is implemented in v0.1 unless it says *later*.

```
file      = header? statement*                  # one statement per line
header    = "easycad" VERSION
statement = project | printer | note | group | param | binding | output

project   = "project" STRING                    # name in the window title and file name
printer   = "printer" STRING                    # default printer profile
note      = "note" STRING                       # free notes, any number of lines
group     = "group" STRING                      # following params go into this group
param     = "param" NAME "=" literal UNIT? modifier*
literal   = "-"? NUMBER | STRING | "true" | "false"
modifier  = "in" NUMBER ".." NUMBER             # slider range, in the setting's own unit
          | "step" NUMBER                       # slider step; typed values snap to it
          | "of" "[" choice ("," choice)* "]"   # a dropdown
          | STRING                              # the label
          | "@" NAME                            # @assumed, @locked, @hidden

binding   = NAME "=" expr                       # one node
output    = "show" NAME ("," NAME)*             # draw these steps
          | "print" expr ("with" expr)?         # the toolpath to print (and the printer)
          | "export" expr "as" STRING           # a file to save: .stl or .gcode (§2.1)

expr      = sum ( "|>" NAME args? )*            # a |> f(x) means f(a, x); a |> f means f(a)
sum       = product (("+" | "-") product)*
product   = unary (("*" | "/") unary)*          # no % operator: % is a unit; use mod(a, b)
unary     = "-" unary | power                   # -t^2 is -(t^2), as in maths
power     = primary ("^" unary)?                # 2^3^2 is 2^(3^2)
primary   = NUMBER UNIT? | STRING | "true" | "false" | NAME | NAME args
          | "[" (expr ("," expr)*)? "]" | "(" expr ")"
args      = "(" (arg ("," arg)*)? ")"
arg       = (NAME ":")? expr                    # positional ones first, then named ones
comment   = "#" … end of line
```

- A statement continues on the next line while brackets are open, or when the next line starts
  with `|>`.
- **Units:** `mm cm m inch` for lengths, `deg rad` for angles (`90°` works too), `%` for ratios
  (18 % is 0.18).
  The engine works in mm, degrees and plain ratios; a bare number is taken as one of those.
  Inches are written `inch` because `in` is the range keyword. A setting keeps its own unit in
  the settings window (18 %, 90°).
- **Reserved words** (they can't name a setting or a step): `easycad project printer note group
  param show export print true false in of step as with`. `pi` is a built-in constant.
- **Limits**, so a hostile file can't hang or crash the app: 256 kB per recipe, expressions
  nested at most 128 deep, lists at most 32 levels deep, at most 4 million points and list
  items per run, and 1 million points per toolpath. Going over gives an error message.
- *Later:* domains as values (`r_min..r_max`), comparisons (`== != < <= > >=`), `and`/`or`/`not`,
  `if … then … else`, record fields (`b.size`) and indexing (`list[0]`), units for speeds and
  temperatures (`mm/s`, `°C`), settings that hold geometry (`param attractor = point(60, 40)`),
  and `of printers()`.

### 2.1 Files: G-code and STL

- `print path` is what the printer prints. **G-code for the chosen printer** (Print tab) is one
  tap away. It is written the way EasyGCode writes it: the same start and end blocks, `G90` with
  relative extrusion (`M83`), E from the bead model (mm³ on the pellet printer, mm of filament on
  the P1P), the part fan from the second layer, coordinates with 3 decimals and E with 5.
  Golden tests compare it with G-code written by EasyGCode itself (`fixtures/easygcode/`).
- `export shape as "shape.stl"` offers a shape (surfaces and solids, or a list of them) as an
  **STL** file, for any slicer or CAD program. `export path as "name.gcode"` offers another
  toolpath as G-code. Other file types are a warning for now (3MF, STEP and DXF come later).
- G-code is checked before it's written: every point must be on the bed and below the build
  height (both from the printer settings), and every extrusion sane. A toolpath that fails gets
  a message instead of a file. The recipe's origin sits at the bed point set in the printer
  settings (EasyGCode's 125/125 by default).

### Project header, presets and conversation
These statements make one file a complete project (decided 14 Oct 2026). `project`, `printer`
and `note` work today; `preset` comes in Phase 1c and `conversation` with the agent (Phase 2):

```
easycad 0.1
project "E27 wavy lampshade"                 # name shown in the library and window title
printer "Klipper pellet"                     # default printer profile for print/gcode
note    "Tested 12 Oct: fit -0.2 is snug"    # free notes, any number of lines

preset "tall"  { height = 240 mm, waves = 12 }   # named sets of settings
preset "test"  { height = 40 mm }                # quick test print

conversation {                               # optional; the engine ignores it
  … the agent chat, stored so it can continue later …
}
```

- **Opening a project** evaluates the recipe with the saved values and shows its settings
  window.
- **Moving a slider** rewrites only that setting's value in its `param` line; comments, layout
  and everything else stay exactly as they were. Saving the project saves that text.
- **Choosing a preset** (later) changes only the values.
- **"Save settings"** writes only the values (a small `.json`, like EasyGCode). "Load settings"
  puts them back into the open project.

---

## 3. Meaning

- **Order doesn't matter, names do.** The file is a graph, not a script. Lines can be in any
  order (the formatter, Phase 1c, will keep them in dependency order). Loops are an error:
  "`a` depends on itself (a → b → a)".
- **No mutation, no loops, no I/O.** Repetition comes from lists. Randomness (later) always
  takes a seed (`random(seed: 7, count: 20)`), so the same file always gives the same result.
- **List replication** (Grasshopper/Dynamo style):
  - every node input is *item* or *list* access;
  - an item input that receives a list makes the node run once per item;
  - with several lists, the longest-list rule applies (the shorter list repeats its last item);
  - `cross(a, b)` gives all combinations (later);
  - operators replicate too: `twist * t` with a list `t` gives a list;
  - list inputs (`loft(curves)`, `union(solids)`, `difference(a, many)`) take the whole list.
- **Records** (later): nodes with several outputs return records. `b = bbox(vase)` gives `b.min`,
  `b.max` and `b.size`; `d = divide(curve, count: 20)` gives `d.points`, `d.tangents` and
  `d.params`. In the graph they are separate output ports.
- **Fields** are numbers that vary over space: `distance(curve)` is how far each point is from
  a curve, and `fade(d, inside: …, outside: …)` turns a distance into a 0..1 weight. Arithmetic
  and the number steps (`min`, `clamp`, `sin`, `smoothstep`, …) work on fields point by point, so
  `line * (1 + near)` is a field too. A step that takes a field (`spiral`'s spacing, `wave`'s
  amplitude) reads it at every point it makes. That's how one setting can affect only part of a
  shape, like Grasshopper's attractors.
- **Errors belong to nodes.** Syntax errors point to a line and column. Mistakes in a step
  ("circle: `r` must be more than 0, got -10") mark that step; steps that need it are skipped
  quietly; everything else still runs and draws. Typos get a suggestion ("unknown name
  `hieght`; did you mean `height`?").
- **Formatting is canonical** (Phase 1c). `easycad fmt` aligns parameter blocks and normalises
  spacing while keeping comments. Agent edits and human edits always look the same, which keeps diffs
  small.

---

## 4. How the text becomes the graph view

| Text | Graph |
|---|---|
| `param height = 160 mm in 60..300 "Height"` | Slider node (left column), also in the Params panel |
| `param attractor = point(60, 40)` | Point node, plus a draggable handle in the viewport |
| `holes = circle(center: grid, r: r) \|> extrude(…)` | One node named **holes**. Wires come in from `grid` and `r`; the pipe chain is shown inside the node as steps |
| `r_max + 2 mm` inside an argument | A small expression badge on that input (like Grasshopper's input expressions) |
| `show`, `export`, `print` | Sink nodes on the right |

Node positions are automatic (left → right by dependency). If you drag nodes around, their
positions are saved in an optional block at the end of the file that the evaluator ignores:

```
layout {
  base: 120 40
  sections: 320 40
}
```

Editing the graph (Phase 5) produces text edits. Text stays the single source of truth.

---

## 5. What the agent gets

The system prompt contains:
- this language summary;
- a **node catalog generated from the code**, so it's always in sync;
- a few examples.

A catalog entry looks like:

```
loft(curves: List<Curve>, degree: Count = 3, closed: Bool = false) -> Surface
    Skins a surface through section curves in order. Sections should share direction and seam.
    See also: sweep, ruled, cap.
```

Details beyond the summary come on demand through the `node_docs` tool, so the prompt stays
small as the library grows.

---

## 6. Node library

### 6.1 Available now (v0.1)

Lengths are in mm and angles in degrees. The app's Recipe tab shows the full catalog with every
input (it's generated from the code, `crates/core/src/nodes.rs`, so it can't go stale).

| Group | Steps |
|---|---|
| Numbers & lists | `range(start, end, count)` evenly spaced numbers, both ends included · `lerp(a, b, t)` · `count(list)` · `min(a, b)` · `max(a, b)` · `clamp(value, low, high)` · `mod(a, b)` · `abs(x)` · `sqrt(x)` · `round(x)` · `floor(x)` · `ceil(x)` · `sin(angle)` · `cos(angle)` · `tan(angle)` · `smoothstep(t)` (an S-curve from 0 to 1). All except `range` and `count` also work on fields |
| Fields | `distance(to)` how far each point is from a curve (negative inside a closed one; exact for circles) · `fade(distance, inside, outside, band)` 1 on the curve, easing to 0 `inside`/`outside` mm away (EasyGCode's spiral-disc falloff) |
| Curves | `circle(r)` · `polygon(sides, r)` · `star(points, outer, inner)` (sharp corners) · `smooth(curve)` (a cubic NURBS through every point) · `spiral(outer, spacing, tolerance, min_samples)` (a flat spiral from the centre out; `spacing` can be a field) · `interlock(spiral, bumps, width)` (wiggles spread-apart turns so each bump meets the next turn's dip) |
| Transforms | `scale(geometry, factor, x, y, z)` · `rotate(geometry, angle)` about the Z axis · `move(geometry, x, y, z)` |
| Surfaces | `loft(sections)` (smooth through the sections; each section's first point is its seam) · `cap(surface)` (flat ends: a solid) |
| Toolpaths | `toolpath(curves, width, layer, speed)` (prints any curves as drawn, in order) · `vase_spiral(wall, layer, width, speed)` (EasyGCode's vase mode: start ramp, whole layers, flat cap turn) · `stack(path, count)` (the same layer `count` times) · `join(paths)` (one after another, each with its own settings) · `wave(geometry, amplitude, count, phase)` (on a toolpath, curve or surface; the amplitude can be a field) |

### 6.2 Planned (≈ 40 nodes for Phases 1–3)

| Group | Nodes |
|---|---|
| Numbers & lists | `range`, `series`, `lerp`, `remap`, `random`, `min`, `max`, `clamp`, `sum`, `count`, `cross`, `flatten`, `zip`, `shift` |
| Points & transforms | `point`, `vector`, `plane`, `move`, `rotate`, `scale`, `mirror`, `orient`, `linear_array`, `polar_array`, `grid_points`, `distance` |
| Curves | `line`, `polyline`, `arc`, `circle`, `ellipse`, `rectangle`, `rounded_rect`, `polygon`, `star`, `interpolate`, `smooth`, `offset`, `fillet`, `divide`, `join`, `reverse`, `seam` |
| Regions (2D) | `region`, `region_union`, `region_difference`, `region_intersection` |
| Surfaces | `extrude`, `revolve`, `loft`, `sweep`, `cap`, `thicken` |
| Solids | `box`, `cylinder`, `sphere`, `union`, `difference`, `intersection`, `hull`, `split`, `bbox`, `volume` |
| Shape → curves | `spiral_on`, `offset_rings`, `contours`, `isocurves`, `project`, `unroll`, `fill` |
| Toolpath & G-code | `printers`, `order`, `travel`, `ramp`, `brim`, `infill`, `overhang_comp`, `check_print` (`toolpath`, `vase_spiral`, `stack`, `join`, `wave` and G-code export are done) |

The printing nodes and how they chain are described in
[toolpaths-and-skills.md](toolpaths-and-skills.md).

---

## 7. Not in v0 (on purpose)

| Feature | Plan |
|---|---|
| User-defined functions | Later: `def leg(h, r) = …`, like Grasshopper clusters |
| Importing other files | Later: `use "parts/hinge.easycad"` |
| Full data trees | Nested lists first. Trees only if real definitions need them |
| Loops, mutation, file/network access | Never |

---

## 8. Open questions

1. **Name and extension.** `.easycad`? (`.ecad` is ambiguous with electronics CAD.)
2. **`|>` pipes vs method chaining** (`base.scale(0.8).rotate(twist)`). Both map to the same
   graph. Pipes make the dataflow explicit; methods feel more familiar. Supporting both is
   cheap.
3. **Variants.** Store named parameter sets ("small", "large") in the file, or as separate
   files?
4. **How much Grasshopper naming to borrow.** Familiar names (`divide`, `loft`, `cull`) help you
   and the agent. Grasshopper's abbreviations (`DivCrv`) don't.
