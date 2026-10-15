# Shapes → curves → toolpaths: how EasyCAD works for printing

*Written 14 Oct 2026 from the owner's answers. This is the core use case and drives the roadmap.*

## The workflow

EasyCAD is **EasyGCode without fixed tabs**. Instead of a new tab being coded for every
project, you (or the agent) build the project inside the app.

1. **Start from a shape or from curves.** A vase, a seat, a lampshade, or a spiral, a profile,
   a pattern. Sometimes the shape comes first, sometimes the curve.
2. **Turn shapes into curves with operations, not with a slicer.** Spiral along the surface,
   follow its own grid lines, offset rings, drape a pattern onto it, unroll it flat.
3. **Turn curves into toolpaths:**
   - bead width and height;
   - speed or volumetric flow;
   - order, seams and travel moves;
   - ramps, brims, overhang compensation.
4. **Turn toolpaths into custom G-code** for your printers (Bambu P1P filament, Klipper pellet).
   No slicer, no 3MF.
5. **Every project has its own settings window,** like an EasyGCode tab. Save the project, reopen
   it later, and the same window comes back with the same settings.

You stay in control: everything the agent builds shows up as settings you can change by hand,
and you can always build or edit things yourself.

---

## Proof: EasyGCode's spiral disc, built inside the app (Phase 1b, 15 Oct 2026)

The owner's question: *"I want to be able to create custom things right out of the thing … and
not have a new app build all the time. Should the architecture not be able to do this?"* It can.
The spiral disc was a dedicated generator inside EasyGCode, written by Claude Code from a long
prompt. In EasyCAD it's a recipe (`web/templates/spiral-disc.easycad`) of general steps, and
its G-code matches EasyGCode's command for command. Each part of the original prompt became a
line:

| The prompt asked for | The recipe line |
|---|---|
| A spiral from the centre out, turns one line width apart, ending on a whole turn | `spiral(outer: outer, spacing: spacing)` |
| A second circle as an attractor that spreads the turns (not the line width) | `distance(circle(ring))` |
| A gradual, Bezier-like falloff with controllable reach, confined to a few turns or spread over everything | `fade(…, inside: fade_in, outside: fade_out, band: band)` |
| Maximum spacing as a % of the line width (200 % = two line widths) | `spacing = line * (1 + (open - 1) * near)` |
| Sine wiggles, an odd count per turn, so each peak meets the next turn's dip with one line width between centrelines, the amplitude following the same falloff | `interlock(disc, bumps: odd, width: line)` |
| An input for the number of identical layers | `toolpath(…) \|> stack(layers)` |

None of these steps is disc-specific: `fade` and `distance` make any setting vary over space,
`spiral` takes any spacing, `stack` repeats any layer. The next project reuses them, so
building it means writing a recipe (by you, the agent, or Copy for AI today), not a new app.
The engine only grows when a genuinely new *kind* of step is needed, and then the step is
written to be reused.

The same goes for plain geometry: `export shape as "name.stl"` saves any surface or solid for a
slicer or CAD program. Booleans, extrusions and fillets are the next geometry steps.

---

## Four layers

| Layer | What it is | Examples |
|---|---|---|
| **Shape** | Surfaces and solids | revolved lampshade, lofted twisted vase, extruded seat outline |
| **Curves** | 2D/3D curves and lists of curves | profile, helix, concentric rings, star outline, wave pattern |
| **Toolpath** | Ordered moves with bead width/height, speed/flow, extrude on/off, fan | one continuous vase spiral, a seat's ring sequence with seams |
| **G-code** | Text for one printer profile | P1P filament (Marlin-style E in mm), Klipper pellet (E as mm³) |

Every operation is a **node**: a precise, tested Rust function. You and the agent use the same
nodes.

---

## Operations (first catalog)

### Make shapes
- `revolve(profile)`, `loft(sections)`, `extrude(outline, height)`, `pipe(curve, r)`.
- **Profile builders from EasyGCode:**
  - `radius_profile` (Catmull-Rom points, as in the vessel tab);
  - `lamp_profile` (throat, fillet, cone/bell/dome/sphere);
  - `thread_tube` (ISO metric fit).

### Shape → curves (the part that replaces slicing)
| Node | What it does | EasyGCode heritage |
|---|---|---|
| `spiral_on(surface, pitch)` | One continuous helix that follows *any* surface (vase mode for every shape, not just prisms) | vessel, lampshade, thread |
| `isocurves(surface, dir, count)` | Follow the surface's own grid lines, which is natural for non-planar paths | — |
| `contours(shape, every)` | Planar sections, only when you actually want layers | — |
| `offset_rings(outline, step)` | Concentric rings with seam styles: staircase, zipper, true spiral | bend stool seat, vessel bottoms |
| `project(curves, onto: surface)` | Drape a flat pattern onto a surface (non-planar decoration) | — |
| `unroll(surface)` / `flatten(shape)` | Lay a (developable) surface out flat: for laser cutting, or print flat and bend afterwards | bend stool idea |
| `fill(outline, pattern)` | Zigzag, spiral or concentric fills | spoon, spiral disc |

### Curve operations
- **Shape edits:** `offset`, `fillet`, `smooth`, `resample` (chord tolerance, like EasyGCode's
  adaptive resolution), `join`, `reverse`.
- **Seam and pattern:** `seam` (choose the start point), `wave` / `spikes` (EasyGCode patterns),
  `attract` (spacing that grows near an attractor, as in the spiral disc), `morph` (blend
  between two curves), `random(seed)`.

### Curves → toolpath
| Node | What it does |
|---|---|
| `toolpath(curves, width, height, speed \| flow)` | Bead model `(w − h)·h + π·(h/2)²`. Width/height can vary per segment (spikes, dome). |
| `order(...)` | Inside-out, outside-in, alternate direction, nearest-next |
| `travel(...)` | Lift/hop rules (EasyGCode's two-move safe travel), retract |
| `ramp(...)` | Extrusion ramp at start/end (vase start, flat cap) |
| `brim(...)`, `prime_line(...)` | As in EasyGCode, including far-to-near print order |
| `overhang_comp(mode, strength)` | Line-width or layer-height mode (lampshade) |
| `fan(...)`, `temps(...)` | Events along the path (fan after first revolution, foam zones) |

### Toolpath → G-code *(done in 1b)*
- `print path` writes G-code for the chosen printer, with EasyGCode's proven start/end blocks
  for **Bambu P1P** and **Klipper pellet**:
  - `G90` absolute positions, `M83` relative extrusion;
  - E is mm³ (pellet) or mm of filament;
  - extrusion multiplier, end lift above the tallest point (never past the build height);
  - the part fan from the second layer.
- **Speeds are set as volumetric flow** (`flow:` in mm³/s): each move's speed is flow ÷ bead
  area, like EasyGCode's flow mode, so the printer's melt rate is what you control. Golden-tested
  against EasyGCode's flow mode (spiral disc and vase). The templates all use it.
- The printer settings (bed size, where the recipe's origin sits, temperatures, fan, purge,
  pressure advance, max flow, …) live in the Print tab, with EasyGCode's defaults.
- Live stats: time, material, flow. Checks: bed fit, height, line width vs nozzle, overhang.
- *Later:* preview coloured by speed (blue = fast, red = slow), drawn on the GPU so huge paths
  stay smooth.

**Porting rule:** every node that comes from EasyGCode gets a **golden test**. With the same
settings, EasyCAD must produce the same G-code as EasyGCode (comments aside). The fixtures are
G-code written by EasyGCode itself (`scripts/easygcode-fixtures.cjs`). The spiral disc matches
in all four cases (flow mode, and pellet, filament and without start/end blocks at a set
speed), and `vase_spiral` matches EasyGCode's vase mode (its lampshade generator, set up as a
plain cylinder) at a set speed and in flow mode. EasyGCode itself is never changed.

---

## Skills: how the agent learns "your ways"

You described this exactly: *"maybe what I'm asking for is actually a set of skills for an AI
agent."* There are three ingredients:

1. **Nodes** are the precise operations (above). Deterministic, tested, the same for you and the
   agent.
2. **Skills** are written know-how: short guides with rules, numbers and an example project.
   - `vase-mode-vessel`: continuous spiral; first revolution ramps extrusion 0 → 100 %; flat cap
     closes the top; line width 1.5–2× nozzle; layer ≤ 75 % of nozzle.
   - `threaded-connector`: layer height = thread pitch (ISO metric); inner edge
     ⌀ = D − 0.5155·P; right-hand thread = counter-clockwise rise.
   - `lamp-holder-mount`: E14 = ⌀28 × 2.0, E27 = ⌀40 × 2.5 (IEC 60399); the throat is the
     thread.
   - `bend-stool-seat`: concentric rings, zipper seam, legs as hairpins, bend-zone spread.
   - `non-planar-pattern`: project a flat pattern onto the surface; overhang limits.
   - `unroll-for-cutting`: unroll, offset for kerf, export the outline.

   The agent sees the list of skill names with one-line descriptions, and loads the full skill
   only when it's relevant. That keeps it fast and cheap (the same idea as Claude's Agent
   Skills).
3. **Your own skills:** "Save as skill" on any project you like. It stores the project plus a
   note about when to use it. Next time, the agent can reuse your recipe instead of starting
   from zero.

Rules that always apply live in two places:
- the **printer profile**: nozzle, bed size, max flow, start/end G-code;
- your **house rules**, e.g. "always ramp extrusion on the first revolution", "pellet: max flow
  60 mm³/s".

---

## Manual control (always)

- **The settings window:** sliders, number fields, toggles, dropdowns, and point editors like
  EasyGCode's drag-editable profile and curve canvases.
- **Edit points and handles directly in the 3D view.**
- **Code view** for exact edits; graph view later.
- The chat box doubles as a command line: `height 200`, `layer 0.4`, `export gcode` happen
  instantly, without AI.

---

## Project files: save and reopen exactly where you were

One project = **one file** (`my-lampshade.easycad`, plain text). It contains:

1. **Project info:** name, notes, printer.
2. **The settings** (current values). This is what builds the settings window.
3. **The recipe** (the nodes).
4. **Presets** (optional): named sets of settings ("small", "tall", "test print").
5. **The conversation** with the agent (optional), so it can continue where you left off. The
   recipe itself already tells a fresh agent most of what it needs.

Opening the file shows the same settings window with the same values, plus the same shape and
toolpath, because they are always regenerated from the recipe.

**Where projects live:**
- the app's own project library (stored in the browser on your device);
- export/import as files: Downloads, the iPhone Files app, iCloud Drive, AirDrop. Share a
  project like any document.

"Save settings" / "Load settings" (only the values, like EasyGCode's JSON files) also works:
it loads values into the project that's open.

---

## A sketch: the lampshade as a project

```
easycad 0.1
project "E27 wavy lampshade"
printer "Klipper pellet"

group "Shape"
param opening   = 220 mm  in 100..400  "Bottom opening ⌀"
param height    = 180 mm  in 60..300   "Height"
param waves     = 9       in 0..24     "Waves per turn"
param wave_size = 6 mm    in 0..20     "Wave size"

group "Mount"
param socket    = "E27"   of ["E14", "E27"]  "Socket"
param fit       = -0.2 mm in -1..1     "Fit tolerance"

profile = lamp_profile(socket, opening: opening, height: height, shape: "bell", fit: fit)
shade   = revolve(profile)
path    = spiral_on(shade, pitch: thread_pitch(socket))     # layer height = thread pitch
          |> wave(amplitude: wave_size, count: waves)
          |> toolpath(width: 2.4 mm, flow: 30 mm3/s)
          |> overhang_comp(mode: "layer", strength: 100 %)

print path
```

Twelve lines replace what was a whole hand-coded tab in EasyGCode, and every number is a
slider.
