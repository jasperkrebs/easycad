# The EasyCAD definition language (draft v0.1)

*Status: draft. The format itself was approved on 14 Oct 2026; syntax and node names will still
change.*

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

## 1. Four examples

### 1.1 Twisted star vase (sliders + list replication)

```
easycad 0.1
# Twisted star vase: a solid for normal printing, plus a direct vase-mode toolpath

group "Shape"
param height    = 160 mm  in 60..300   "Height"
param radius    = 45 mm   in 15..120   "Base radius"
param points    = 7       in 3..16     "Star points"
param depth     = 18 %    in 0..40     "Star depth"
param twist     = 90 deg  in 0..360    "Twist"
param top_scale = 0.8     in 0.3..1.5  "Top scale"

group "Print"
param layer     = 0.3 mm  in 0.1..1.2  "Layer height"
param line      = 0.6 mm  in 0.3..2    "Line width"

base     = star(points, outer: radius, inner: radius * (1 - depth)) |> smooth(0.5)
t        = range(0, 1, count: 32)                  # 32 numbers from 0 to 1
sections = base
           |> scale(lerp(1, top_scale, t))         # t is a list, so this makes 32 curves
           |> rotate(twist * t)
           |> move(z: height * t)
wall     = loft(sections)                          # NURBS surface through all sections
vase     = wall |> cap()                           # close both ends: a watertight solid

path     = vase_spiral(wall, layer: layer, width: line)   # one continuous helix along the wall

show vase
export vase as "twisted-vase.3mf"
print path
```

What you get in the app:
- two slider groups (Shape, Print);
- a graph of 8 nodes;
- a solid to export;
- a toolpath to print directly.

Asking the agent for "less twist at the bottom" changes one line (`rotate(twist * t^2)`).
Making it taller is a slider.

### 1.2 Attractor panel (the classic Grasshopper demo)

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

### 1.4 Printing a curve directly (EasyGCode heritage)

```
easycad 0.1
# Woven cylinder printed straight from a curve: no slicer

param radius  = 60 mm   in 20..150  "Radius"
param height  = 120 mm  in 20..300  "Height"
param bumps   = 9       in 3..41    "Bumps per turn"   # odd numbers weave, even numbers make flutes
param amp     = 3 mm    in 0..10    "Bump size"

group "Print"
param printer = "Bambu P1P 0.4" of printers()  "Printer"
param layer   = 0.3 mm  in 0.1..1   "Layer height"
param line    = 0.6 mm  in 0.3..2   "Line width"

helix = vase_spiral(circle(r: radius), height: height, layer: layer)  # a single 3D curve
wavy  = helix |> wave(amplitude: amp, per_turn: bumps)                # displaced along its normal
path  = toolpath(wavy, width: line, height: layer)                    # bead model → extrusion

print path with printer
```

This is EasyGCode's model: geometry *is* the toolpath. Extrusion comes from the bead
cross-section (`(w − h)·h + π·(h/2)²`), and the preview is coloured by speed.

---

## 2. Syntax (sketch)

```
file      = header? statement*
header    = "easycad" VERSION
statement = group | param | binding | output | layout | comment

group     = "group" STRING                      # following params go into this group
param     = "param" NAME "=" value range? step? choices? STRING? attribute*
range     = "in" NUMBER ".." NUMBER
step      = "step" NUMBER
choices   = "of" ( "[" value ("," value)* "]" | call )
attribute = "@assumed" | "@locked" | "@hidden"

binding   = NAME "=" expr                       # one node
output    = "show" NAME ("," NAME)*
          | "export" expr "as" STRING
          | "print" expr ("with" expr)?

expr      = domain ( "|>" call )*               # a |> f(x) means f(a, x)
domain    = sum ( ".." sum )?                   # 1..5 is a Domain value
sum       = product (("+" | "-") product)*
product   = power (("*" | "/") power)*            # no % operator: % is a unit; use mod(a, b)
power     = unary ("^" unary)?
unary     = ("-" | "not")? postfix
postfix   = primary ( "." NAME | "[" expr "]" )*  # record fields, list indexing
primary   = NUMBER UNIT? | STRING | "true" | "false" | NAME | call | list
          | "(" expr ")" | "if" expr "then" expr "else" expr
call      = NAME "(" ( arg ("," arg)* )? ")"
arg       = (NAME ":")? expr                     # positional first, then named
list      = "[" ( expr ("," expr)* )? "]"
comment   = "#" … end of line
```

- Comparison (`== != < <= > >=`) and boolean (`and`, `or`) operators exist too, with the usual
  precedence.
- One statement per line. A line may continue when the next line starts with `|>` or when
  brackets are still open.

**Units:** `mm cm m in ft` for length and `deg rad` for angles; `%` means ÷100. A bare number
in a length slot is mm, in an angle slot degrees. Speeds and temperatures (`mm/s`, `°C`)
arrive with the print nodes.

### Project header, presets and conversation
These statements make one file a complete project (decided 14 Oct 2026):

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
- **Choosing a preset** changes only the values.
- **"Save settings"** writes only the values (a small `.json`, like EasyGCode). "Load settings"
  puts them back into the open project.

---

## 3. Meaning

- **Order doesn't matter, names do.** The file is a graph, not a script. Lines can be in any
  order; the formatter keeps them in dependency order. Cycles are an error.
- **No mutation, no loops, no I/O.** Repetition comes from lists. Randomness always takes a
  seed (`random(seed: 7, count: 20)`), so the same file always gives the same result.
- **List replication** (Grasshopper/Dynamo style):
  - every node input is *item* or *list* access;
  - an item input that receives a list makes the node run once per item;
  - with several lists, the longest-list rule applies (the shorter list repeats its last item);
  - `cross(a, b)` gives all combinations;
  - operators replicate too: `twist * t` with a list `t` gives a list;
  - list inputs (`loft(curves)`, `union(solids)`, `difference(a, many)`) take the whole list.
- **Records:** nodes with several outputs return records. `b = bbox(vase)` gives `b.min`,
  `b.max` and `b.size`; `d = divide(curve, count: 20)` gives `d.points`, `d.tangents` and
  `d.params`. In the graph they are separate output ports.
- **Errors belong to nodes.** Syntax errors point to a line and column with a suggestion.
  Geometry errors ("loft failed: section 12 is reversed") mark that node red. Everything that
  doesn't depend on it still evaluates.
- **Formatting is canonical.** `easycad fmt` aligns parameter blocks and normalises spacing
  while keeping comments. Agent edits and human edits always look the same, which keeps diffs
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

## 6. Node library v0 (≈ 40 nodes for Phases 1–3)

| Group | Nodes |
|---|---|
| Numbers & lists | `range`, `series`, `lerp`, `remap`, `random`, `min`, `max`, `clamp`, `sum`, `count`, `cross`, `flatten`, `zip`, `shift` |
| Points & transforms | `point`, `vector`, `plane`, `move`, `rotate`, `scale`, `mirror`, `orient`, `linear_array`, `polar_array`, `grid_points`, `distance` |
| Curves | `line`, `polyline`, `arc`, `circle`, `ellipse`, `rectangle`, `rounded_rect`, `polygon`, `star`, `interpolate`, `smooth`, `offset`, `fillet`, `divide`, `join`, `reverse`, `seam` |
| Regions (2D) | `region`, `region_union`, `region_difference`, `region_intersection` |
| Surfaces | `extrude`, `revolve`, `loft`, `sweep`, `cap`, `thicken` |
| Solids | `box`, `cylinder`, `sphere`, `union`, `difference`, `intersection`, `hull`, `split`, `bbox`, `volume` |
| Shape → curves | `spiral_on`, `offset_rings`, `contours`, `isocurves`, `project`, `unroll`, `fill` |
| Toolpath & G-code | `printers`, `vase_spiral`, `toolpath`, `order`, `travel`, `ramp`, `wave`, `brim`, `overhang_comp`, `gcode`, `check_print` |

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
