# The EasyCAD definition language (v0.1)

*Status: **v0.1 is implemented** (Phase 1a, 15 Oct 2026): the parser is `crates/lang`, the
evaluator and nodes are `crates/core`. Phase 1b (15 Oct 2026) added fields (numbers that vary
over space), general toolpath steps, and files: G-code and STL (§2.1). Phase 1c added the
Grasshopper-style toolbox: points and planes, the coordinate fields `x`, `y`, `z`, 3D transforms
that take fields (a different move, turn or scale for every point), curve and list tools,
surfaces from curves, and `spiralize` (vase mode through any layers, tilted ones included). §2
says exactly which syntax works today and what comes later; §6 lists the steps you can use, with
their Grasshopper names. The format was approved
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

### 1.6 A cylinder cut by a tilted plane (tilted layers)

The fifth template (`web/templates/cut-cylinder.easycad`). The top of a cylinder is cut at an
angle. The layers in between blend from the flat bottom to the slanted top, like Tween Curve in
Grasshopper. They are printed as one spiral, each move extruded for its own layer height:

```
base   = circle(diameter / 2)
cut    = plane([0, 0, height]) |> rotate(angle, axis: [0, 1, 0], center: [0, 0, height])
top    = base |> project_along(cut)
layers = tween(base, top, range(0, 1, count: round(height / layer) + 1))
path   = spiralize(layers, width: line, flow: flow)
```

Read it like a Grasshopper definition:
- **Circle** → `base`.
- **Plane Normal** at the height, then **Rotate 3D** about Y → `cut`.
- **Project Along** Z onto it → `top`, the slanted top edge.
- **Tween Curve** with a **Range** of factors → `layers`.

`spiralize` blends each layer into the next in one continuous spiral (vase mode). Layers are
thin on the low side and thick on the high side, and every move is extruded for its own height.
The flow (mm³/s) stays the same, so the thin side is printed faster. With the angle at 0 it is
ordinary vase mode, and through flat, evenly spaced layers `spiralize` prints exactly like
`vase_spiral` (both golden-tested against EasyGCode). Through tilted layers it is checked
against a hand-checked fixture (`fixtures/hand-checked/`).

The same toolbox does it another way too: `move(z: height + tan(angle) * x)` lifts every point
of the circle by its own amount (a *field*, §3). It lands on the same slanted top.

### 1.7 Geometry only

Not every project is printed by EasyCAD. A recipe without `print` is plain geometry: shown,
and saved with `export … as "….stl"` for a slicer or another program. The sixth template
(`web/templates/turned-vase.easycad`) turns a profile through five points around the Z axis:

```
profile = interpolate([[foot, 0, 0], [belly, 0, height * belly_at], [neck, 0, height * neck_at], [lip, 0, height]])
body    = revolve(profile)
solid   = cap(body)

show solid, profile
export solid as "turned-vase.stl"
```

`[foot, 0, 0]` is a point written as a list. Every radius and height is a setting, like number
sliders feeding Construct Point.

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
  param show export print true false in of step as with`.
- **Built-in names:** `pi` is 3.14159…, and `x`, `y`, `z` are the coordinates of each point as
  fields (§3). A setting or step with the same name wins.
- **Points** are written `point(x, y, z)` or as a list of 2 or 3 numbers, `[x, y, z]`, wherever a
  point or vector is asked for. `[[0, 0], [10, 0], [10, 10]]` is a list of points.
- **Limits**, so a hostile file can't hang or crash the app: 256 kB per recipe, expressions
  nested at most 128 deep, lists at most 32 levels deep, at most 4 million points and list
  items per run, and 1 million points per toolpath. Going over gives an error message.
- *Later:* domains as values (`r_min..r_max`), comparisons (`== != < <= > >=`), `and`/`or`/`not`,
  `if … then … else`, record fields (`b.size`) and indexing (`list[0]`), units for speeds and
  temperatures (`mm/s`, `°C`), settings that hold geometry (`param attractor = point(60, 40)`),
  and `of printers()`.

### 2.1 Files: G-code and STL

**Speed as volumetric flow.** The toolpath steps take `flow:` in mm³/s (a setting like
`param flow = 8 in 1..25 "Flow (mm³/s)"`). Each move's speed then follows from its bead:
speed = flow ÷ bead area, and the G-code feed is flow · 60 ÷ area, rounded, exactly like
EasyGCode's flow mode (golden-tested). A thicker bead is printed slower, so the printer's
melt rate is what you set. `speed:` (mm/s) still works when you want a fixed speed. The
Print tab checks the flow against the printer's "max flow" setting.

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
  - list inputs (`loft(curves)`, `polyline(points)`) take the whole list;
  - a list of lists works like a Grasshopper tree: a list input runs once per inner list
    (`count([[1, 2], [3]])` is `[2, 1]`), and `flatten` makes it one list;
  - a point input reads `[x, y, z]` as one point, and runs once per point for a list of them.
- **Records** (later): nodes with several outputs return records. `b = bbox(vase)` gives `b.min`,
  `b.max` and `b.size`; `d = divide(curve, count: 20)` gives `d.points`, `d.tangents` and
  `d.params`. In the graph they are separate output ports.
- **Fields** are numbers that vary over space: `distance(curve)` is how far each point is from
  a curve, and `fade(d, inside: …, outside: …)` turns a distance into a 0..1 weight. Arithmetic
  and the number steps (`min`, `clamp`, `sin`, `smoothstep`, …) work on fields point by point, so
  `line * (1 + near)` is a field too. A step that takes a field (`spiral`'s spacing, `wave`'s
  amplitude) reads it at every point it makes. That's how one setting can affect only part of a
  shape, like Grasshopper's attractors.
- **`x`, `y` and `z` are fields** too: each point's own coordinates. Given a field, the
  transforms work point by point:
  - `move(c, z: 0.1 * x)` tilts a flat curve into a slope;
  - `rotate(wall, z * 0.5)` twists 0.5° per mm of height;
  - `scale(wall, 1 + 0.002 * z)` flares a wall upward.

  In Grasshopper you would Deconstruct Point, do the maths and Construct Point again; here the
  expression is the maths. `sample(field, point)` reads a field's value at one point.
- **Errors belong to nodes.** Syntax errors point to a line and column. Mistakes in a step
  ("circle: `r` must be more than 0, got -10") mark that step; steps that need it are skipped
  quietly; everything else still runs and draws. Typos get a suggestion ("unknown name
  `hieght`; did you mean `height`?").
- **Formatting is canonical** (Phase 1c). `easycad fmt` aligns parameter blocks and normalises
  spacing while keeping comments. Agent edits and human edits always look the same, which keeps diffs
  small.

---

## 4. How the text becomes the graph view

The Graph tab (since 15 Oct 2026) draws the recipe the way Grasshopper draws a definition, and
every change made there is an edit of this text:

| Text | Graph |
|---|---|
| `param height = 160 mm in 60..300 "Height"` | A slider component in the left column (also in the Settings tab) |
| `outline = star(points, outer: radius) \|> smooth` | One component named **outline**. Its stages are listed inside it (`star`, `\|> smooth`), each with its inputs; a wire comes in from `points` and from `radius` |
| `inner: radius * (1 - depth)` | The expression is shown on that input, with wires from `radius` and `depth` (like an expression on a Grasshopper input) |
| `sections = outline \|> scale(…)` | A first row `= outline`, wired from **outline**; the piped input of `scale` isn't shown separately |
| `show a, b` | The preview dot on components **a** and **b** |
| `print`, `export` | Sinks on the right |

What the graph's commands do to the text (`easycad_core::graph`; each result is parsed before
it's used, and only the piece that changes is touched):

- **Type or wire an input:** replaces that argument's text, or adds `name: value`; a wire is
  just the name. Clearing an input removes its argument; positional values after it become
  named, so they keep feeding the same inputs.
- **Add a component:** a new line `offset1 = offset(outline, 10)`, after the step it's fed
  by. Required inputs with an obvious value get one; the others stay missing (red) until wired.
- **Add a stage:** appends `|> name(…)`. **Remove a stage** takes it off again.
- **Remove a component or setting:** deletes its line, empties the inputs wired only to it,
  and takes it out of `show`.
- **Rename:** the definition and every use (not argument names, not calls).
- **Preview:** adds the step to `show`, or removes it.
- **Add or change a slider:** writes the `param` line (value, range, step, label).

Positions are automatic: left to right by dependency, settings first. If you drag components,
their positions are kept on the device for now; later they can go into an optional block at the
end of the file that the evaluator ignores:

```
layout {
  base: 120 40
  sections: 320 40
}
```

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
| Numbers & lists | `range(start, end, count)` evenly spaced numbers, both ends included · `series(start, step, count)` · `random(count, low, high, seed)` (the same numbers for the same seed) · `count(list)` · `item(list, index)` (wraps: -1 is the last) · `reverse(list)` · `shift(list, offset)` · `flatten(list)` · `pick(test, yes, no)` (`yes` when the test is true or a number other than 0: `pick(line, line, 2.4 * pitch)` is "0 means auto") · `index_of(list, value)` (where a value is, counting from 0: `item(pitches, index_of(sizes, size))` looks a dropdown up in a table) · `sum(list)` · `accumulate(list)` (running totals: layer heights in, layer positions out) · `bounds(list)` ([smallest, largest]) · `graft(list)` (each item in a list of its own, so a whole-list step runs once per item) · `graph(xs, ys, x)` (a piecewise-linear table read at `x`; a field for `x` gives a field: Graph Mapper) · `lerp(a, b, t)` · `remap(value, from_low, from_high, to_low, to_high)` · `min(a, b)` · `max(a, b)` · `clamp(value, low, high)` · `mod(a, b)` · `abs(x)` · `sqrt(x)` · `round(x)` · `floor(x)` · `ceil(x)` · `sin(angle)` · `cos(angle)` · `tan(angle)` · `asin(x)` · `acos(x)` · `atan(x)` · `atan2(y, x)` · `exp(x)` · `ln(x)` · `sign(x)` · `smoothstep(t)` (an S-curve from 0 to 1) · `greater(a, b)` and `less(a, b)` (1 or 0: masks and tests, `greater(z, 2)`). The maths steps (from `lerp` on) also work on fields |
| Fields | `x`, `y`, `z` (each point's coordinates) · `distance(to)` how far each point is from a curve (negative inside a closed one; exact for circles) or from a point · `fade(distance, inside, outside, band)` 1 on the curve, easing to 0 `inside`/`outside` mm away (EasyGCode's spiral-disc falloff) · `lean(wall)` how far a wall leans from vertical at each point, in degrees (positive: an overhang) |
| Points & planes | `point(x, y, z)` · `vector(x, y, z)` · `plane(origin, normal)` · `sample(field, at)` (a field's value at a point) · `scatter(width, height, count, seed, relax)` (points spread evenly but irregularly over a rectangle: blue noise, EasyGCode's spike placement) |
| Curves | `circle(r, tolerance)` (with a tolerance, enough points that no flat strays more than that from the true circle, EasyGCode's chord rule) · `ellipse(rx, ry)` · `superellipse(rx, ry, exponent)` (EasyGCode's squircle) · `rectangle(width, height, radius)` · `arc(r, start, end, tolerance)` (with a tolerance, EasyGCode's chord rule, at least 12 segments) · `vertices(curve)` (its points, as a list) · `line(start, end)` · `polyline(points, closed)` · `interpolate(points, closed)` (smooth, through every point) · `catmull_rom(points, closed, samples)` (EasyGCode's spline) · `polygon(sides, r)` · `star(points, outer, inner)` (sharp corners) · `smooth(curve)` (a cubic NURBS through every point) · `spiral(outer, spacing, tolerance, min_samples, start, turns)` (a flat spiral: from the centre out to near `outer`, `spacing` can be a field; or exactly `turns` turns from radius `start`) · `interlock(spiral, bumps, width)` (wiggles spread-apart turns so each bump meets the next turn's dip) · `tween(a, b, t)` · `offset(curve, distance)` (the distance can be a field) · `fillet(curve, radius)` (corners rounded with arcs) · `simplify(curve, tolerance)` · `subcurve(curve, start, end)` · `join_curves(curves, tolerance)` · `divide(curve, count)` (points) · `evaluate(curve, t)` (a point) · `tangent(curve, t)` and `normal(curve, t)` (directions; the normal is outward on a closed curve) · `closest(curve, point)` (where along it) · `centroid(curve)` · `length(curve)` · `flip(curve)` · `seam(curve, t)` · `resample(curve, count)` · `rings(curve, spacing, inset)` (rings inside an outline, EasyGCode's ring fill) · `link_rings(rings, style, width)` (rings joined into one line: `"staircase"`, `"zipper"` or `"spiral"`) |
| Transforms | `scale(geometry, factor, x, y, z, center)` · `rotate(geometry, angle, axis, center)` · `move(geometry, x, y, z)` (all three take fields: per point) · `mirror(geometry, plane)` · `project_along(geometry, plane, direction)` · `center(geometry)` (centred on the bed together, `center(rotate(c, 45))`). They work on curves, surfaces, points and planes. On toolpaths, only what keeps layers level: fixed moves and scales, turns about the vertical, mirrors through upright planes |
| Surfaces | `loft(sections)` (smooth through the sections; each section's first point is its seam) · `cap(surface)` (flat ends: a solid) · `extrude(curve, height)` · `revolve(profile)` (about the Z axis) · `contour(surface, layer, pace)` (flat layer curves, as `vase_spiral` prints them; with `pace`, closer where the wall leans: EasyGCode's overhang-paced layers) |
| Toolpaths | `toolpath(curves, width, layer, speed, flow)` (prints any curves as drawn, in order) · `vase_spiral(wall, layer, width, speed, flow, flat_top)` (EasyGCode's vase mode: start ramp, whole layers, a flat closing turn or an open top) · `spiralize(layers, width, speed, flow, flat_top)` (vase mode through any stack of closed layers, flat or tilted; each move extruded for its own layer height) · `helix(circle, pitch, height, width, speed, flow)` (vase mode around a circle, exactly one pitch per turn to exactly the height: EasyGCode's thread connector) · `vase_revolve(profile, layer, width, speed, flow, bead_layer, tolerance, seam)` (vase mode around the Z axis following a profile of revolution, turn by turn: rise, width and flow can be fields read once per turn; EasyGCode's lampshade) · `stack(path, count, lift, wipe)` (the same layer `count` times; the travel back `lift` layers up, 0 straight over, or with `wipe` one straight move to that far above the next start and down) · `join(paths, clear, wipe)` (one after another, each with its own settings; up, over and down between parts, at least `clear` above the bed, or a wipe; parts that meet carry straight on) · `bead(path, width, layer)` (the same moves extruded for another bead) · `adjust(path, speed, extrude)` (speed and extrusion by zone, from fields) · `accelerate(path, acceleration)` (speed that ramps up instead of jumping) · `fan(path, level)` (part fan levels by zone) · `dwell(path, at, seconds)` (pauses) · `temperature(path, nozzle, up, mid, down, at_end)` (new heater temperatures before the path prints or after it; nothing given is the printer's own) · `wait_temperature(path, nozzle, up, mid, down, cool, at_end)` (waits until hot or cool enough, within 2 °C) · `speed_override(path, percent, at_end)` and `extrusion_override(path, percent, at_end)` (the printer's `M220` and `M221`: EasyGCode's foaming) · `spikes(path, at, width, length, angle, out_speed, tip_speed, in_speed, dwell)` (EasyGCode's staples) · `wave(geometry, amplitude, count, phase)` (on a toolpath, curve or surface; the amplitude can be a field) |

**For Grasshopper users.** The steps are named after Grasshopper's components, in lower case
(each step's description in the app gives the component it matches):

| Grasshopper | EasyCAD |
|---|---|
| Number Slider, Boolean Toggle, Value List, Panel | `param` lines: slider, toggle, dropdown, text |
| Construct Point, Vector XYZ, Plane Normal | `point`, `vector` (or `[x, y, z]`), `plane` |
| Deconstruct Point, Evaluate Field | the fields `x`, `y`, `z`; `sample` |
| Range, Series, Random, List Item, List Length, Reverse List, Shift List, Flatten | `range`, `series`, `random`, `item`, `count`, `reverse`, `shift`, `flatten` |
| Remap Numbers, Expression | `remap`; any arithmetic inside an argument |
| Circle, Ellipse, Rectangle, Arc, Line, Polyline, Interpolate, Polygon | `circle`, `ellipse`, `rectangle`, `arc`, `line`, `polyline`, `interpolate`, `polygon` |
| Tween Curve, Offset Curve, Divide Curve, Evaluate Length, Length, Flip Curve, Seam, Rebuild | `tween`, `offset`, `divide`, `evaluate`, `length`, `flip`, `seam`, `resample` |
| Move, Rotate 3D, Scale / Scale NU, Mirror, Project Along | `move`, `rotate`, `scale`, `mirror`, `project_along` (and they take fields: an attractor per point) |
| Loft, Cap Holes, Extrude, Revolution, Contour | `loft`, `cap`, `extrude`, `revolve`, `contour` |
| Graph Mapper, attractor definitions | `smoothstep`, `fade`, `distance` and arithmetic on fields |
| Pick'n'Choose, Member Index, Mass Addition, Larger Than / Smaller Than | `pick`, `index_of`, `sum` and `accumulate`, `greater` and `less` |
| Fillet, Reduce, Sub Curve, Join Curves, Evaluate Curve, Curve Closest Point, Area (centroid) | `fillet`, `simplify`, `subcurve`, `join_curves`, `tangent` and `normal`, `closest`, `centroid` |
| Populate 2D | `scatter` |

Not there yet: data trees beyond nested lists, Brep solids and booleans, Sweep, Pipe, curve
booleans, Graph Mapper's curve editor, and clusters (§7).

**For EasyGCode users.** Every EasyGCode project can be built from these steps; which step does
what, project by project, is in [easygcode-coverage.md](easygcode-coverage.md). The thread
connector and the spoon ship as templates whose G-code is identical to EasyGCode's.

### 6.2 Planned (≈ 40 nodes for Phases 1–3)

| Group | Nodes |
|---|---|
| Numbers & lists | `cross`, `zip`, `cull`, `sort`, a `graph` from a drawn curve |
| Points & transforms | `orient`, `linear_array`, `polar_array`, `grid_points`, `closest_point` |
| Curves | `fillet`, `join_curves`, `explode`, `curve_curve` (intersections), `trim`, `split` |
| Regions (2D) | `region`, `region_union`, `region_difference`, `region_intersection` |
| Surfaces | `sweep`, `pipe`, `thicken`, `isocurves`, `surface_point` |
| Solids | `box`, `cylinder`, `sphere`, `union`, `difference`, `intersection`, `hull`, `split`, `bbox`, `volume` |
| Shape → curves | `spiral_on`, `offset_rings`, `unroll`, `fill` |
| Toolpath & G-code | `printers`, `order`, `travel`, `ramp`, `brim`, `infill`, `overhang_comp`, `check_print` |

Done since the first plan: `series`, `random`, `remap`, `flatten`, `shift`, `point`, `vector`,
`plane`, `mirror`, `line`, `polyline`, `arc`, `ellipse`, `rectangle` (with rounded corners),
`interpolate`, `offset`, `divide`, `seam`, `flip`, `extrude`, `revolve`, `contour`,
`project_along`, `spiralize`, and the toolpath steps with G-code export. For EasyGCode's
projects (17 Oct 2026): `sum` (and `accumulate`), `fillet`, `join_curves`, `closest`,
`offset_rings` (as `rings` + `link_rings`), `overhang_comp` (as `lean` + `adjust`, and
`contour`'s `pace`), `ramp` (as `accelerate`), and more: `pick`, `index_of`, `greater`, `less`,
`scatter`, `superellipse`, `catmull_rom`, `simplify`, `subcurve`, `tangent`, `normal`,
`centroid`, `center`, `helix`, `bead`, `fan`, `dwell`, `spikes`.

The printing nodes and how they chain are described in
[toolpaths-and-skills.md](toolpaths-and-skills.md).

---

## 7. Not in v0 (on purpose)

| Feature | Plan |
|---|---|
| User-defined functions | Later: `def leg(h, r) = …`, like Grasshopper clusters |
| Importing other files | Later: `use "parts/hinge.easycad"` |
| Full data trees | Nested lists work as branches (a list input runs once per inner list; `flatten`). Full trees with paths only if real definitions need them |
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
