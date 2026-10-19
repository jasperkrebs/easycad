# EasyCAD roadmap

**Rule:** every phase ends with something you can use. We don't start a phase until the previous
one is useful. No rocket ship, just one working step at a time.

**Focus (from your answers, 14 Oct 2026):** custom toolpaths and G-code for your own printers.
Shapes → curves → toolpaths, each project with its own settings window, and an AI agent that
builds those projects with you. Normal slicing, 3MF and Orca/Bambu Studio workflows are
**not** a priority. See [toolpaths-and-skills.md](toolpaths-and-skills.md).

Sizes are relative (S ≈ one focused session, M ≈ a few, L ≈ many).

---

## Decisions (answered 14 Oct 2026)

| # | Question | Decision |
|---|---|---|
| 1 | UI stack | **Rust engine (WebAssembly) + plain web interface.** EasyCAD is a web app: it runs in the browser on computer and phone and can be installed from the browser (PWA). A desktop wrapper is optional, later. |
| 2 | Project format | **Own small text format (`.easycad`).** One file holds the recipe (nodes), all settings, presets and optionally the agent conversation. Opening it brings back the same settings window. |
| 3 | License | **All rights reserved** (no open-source license). Everything we build on is permissively licensed (MIT/Apache/BSD; MPL is fine), so this limits nothing; we keep their license notices. The repo is *public* since 15 Oct (see hosting), which makes the code visible but doesn't grant anyone a license to reuse it. |
| 4 | Printers | **Bambu P1P (filament) and Klipper pellet**, with EasyGCode's proven start/end G-code. The pellet printer is the **Ginger Additive G1** (18 Oct): made for it, its specifics kept as data so another Klipper printer can follow ([printers.md](printers.md)). |
| 5 | AI | **Built into the app with your own Claude API key** (standalone; see [api-key.md](api-key.md)). Claude Desktop/MCP control is optional, much later. |
| 6 | Hosting (15 Oct) | **Public repo + GitHub Pages** (free), live at https://jasperkrebs.github.io/easycad/. CI deploys on every push to the default branch. Can go private + GitHub Pro later. |
| 7 | Making shapes (15 Oct) | Recipes, not "AI every time": the agent (or you) writes a recipe once; it then runs locally, free and instantly. Five ways to make one: templates, describe, steps, draw curves, import. See [creating-shapes.md](creating-shapes.md). |
| 8 | Custom projects (15 Oct) | **General steps, not one generator per project.** "I want to create custom things right out of the app", both G-code projects like EasyGCode's and plain geometry to export. So EasyGCode's projects become recipes made of general steps (fields, spirals, toolpaths, …), and the app only changes when a genuinely new *kind* of step is needed. Shown first with the spiral disc (Phase 1b). |
| 9 | The AI in the app (15 Oct) | **Your own key, never on the site:** either a password for your own small proxy (a Cloudflare Worker that holds the key, deployed by CI from GitHub secrets), or the key stored on one device. **Sonnet 5.5 at medium effort** by default (Opus 5.5 for hard inventions, Haiku 5.5 for quick edits), with the prompt cached. Memory: `note` lines in the project, the chat on the device, and "Your rules". See [api-key.md](api-key.md). |
| 10 | Speeds (15 Oct) | **Always as volumetric flow** (mm³/s): each move's speed follows from its bead, as in EasyGCode's flow mode. |
| 11 | The toolbox (15 Oct) | **A Grasshopper vocabulary, not per-project tools**, and a **Graph tab** to build and edit recipes as components and wires without AI. The AI wires; you keep control of the design. |

---

## Phase 0: Walking skeleton (S) ✓ *done 14 Oct 2026 (hosting pending)*

What landed and the numbers are in [devlog.md](devlog.md).
- **The app shell** in the EasyGCode look (PWA: installable, offline).
- **The Rust engine** compiled to WebAssembly and running in a background worker, so the UI
  never freezes.
- **A GPU 3D viewport** (three.js, WebGL2 now; WebGPU later).
- **Demo:** the twisted star vase, with its spiral toolpath, sliders and print stats, computed
  by the Rust engine (NURBS via `curvo`).
- **CI:** formatting, lints (no-panic rules), tests, WebAssembly build with a size limit, and
  deploy as soon as hosting is decided.

**Done when:** you open the app on your phone and computer, drag sliders, and see the shape
and its toolpath update instantly.

## Phase 1: Grasshopper-lite + projects (M–L) ← *in progress*

Split into small steps (15 Oct):
- **1a: Recipes become files** ✓ *done 15 Oct 2026* (see [devlog.md](devlog.md))
  - Language v0.1 parser and evaluator, plus the first 24 nodes.
  - The twisted vase rebuilt as a recipe (prints the same as Phase 0); two more templates.
  - Settings window generated from the recipe; Recipe tab (with "Copy for AI"); open/save
    `.easycad`; autosave.
- **1b: Real G-code** ✓ *done 15 Oct 2026* (see [devlog.md](devlog.md))
  - G-code for the P1P and the Klipper pellet printer in EasyGCode's dialect, with its start and
    end blocks, golden-tested against G-code written by EasyGCode itself; printer settings in
    the Print tab; bed and height checks before anything is written.
  - General steps: fields (`distance`, `fade`, maths on fields), `spiral`, `interlock`,
    `toolpath`, `stack`, `join`.
  - **EasyGCode's spiral disc as a recipe** of those steps: its G-code matches EasyGCode's,
    command for command.
  - STL export (`export shape as "x.stl"`) for plain geometry.
- **1c: The Grasshopper toolbox and the Graph tab** ✓ *done 15 Oct 2026* (see
  [devlog.md](devlog.md))
  - About 70 general steps named after Grasshopper's components: points, planes, list tools,
    curve tools (`tween`, `offset`, `divide`, …), transforms that take fields (per point) and
    any axis, `extrude`, `revolve`, `contour`, and `spiralize` (vase mode through tilted
    layers, hand-checked fixture). Speeds as volumetric flow, golden-tested.
  - Templates: "Cut cylinder" (tilted layers) and "Turned vase" (geometry only).
  - The Graph tab: the recipe as components and wires, editable without AI.
- **1d: Everything EasyGCode does, from general steps** ✓ *done 17 Oct 2026* (see
  [easygcode-coverage.md](easygcode-coverage.md))
  - All eight EasyGCode projects audited feature by feature; about 35 general steps added so
    every one can be built as a recipe (ring fills, `helix`, zone speeds and extrusion,
    acceleration ramps, fan levels, dwells, spikes, overhang-paced layers, …). The one gap
    left is the bend stool's foaming (printer heater macros).
  - The thread connector and the spoon as templates: their G-code matches EasyGCode's,
    command for command (three cases each).
  - The vessel, lampshade, container, coat hanger and bend stool are recipe work now, written
    when wanted (by hand or by the agent).
- **1d+: Feature requests** ✓ *built 17 Oct 2026, waiting for its one-time setup*: when the
  in-app agent can't build something, it files a request; a Claude Code session builds the step,
  the workflow checks and ships it, and the chat carries on
  ([feature-requests.md](feature-requests.md)).
- **1e: EasyGCode's projects as templates** ✓ *mostly done 17 Oct 2026* (see
  [devlog.md](devlog.md))
  - The bend stool with **foaming** (new machine-event steps: temperatures, waits, speed and
    extrusion overrides) and the **lampshade** with every shade (`vase_revolve`): their G-code
    matches EasyGCode's, command for command (four cases each).
  - The **vessel** from general steps: the bottom rounding into the wall through a fillet, a
    star top with lifted corners (non-planar), closer layers on overhangs.
  - The agent can switch printers and change printer settings (`set_printer`).
  - The pellet printer is the **Ginger Additive G1**, from its wiki: its bed, flow window,
    heater limits (the engine refuses G-code over them), material presets and checks
    ([printers.md](printers.md)).
- **1f: the container** ✓ *19 Oct 2026*, as the owner's **salt & pepper shaker**: a container and
  a lid that threads onto it, two prints from one window. For it: projects that **print several
  parts** (a G-code file, estimate and checks each), **list settings** (a table of rows, like
  radii at heights) and a **smooth `graph`** (through its points, level at both ends).
- **1g** ← *next*: the coat hanger as a template; formatter, presets, project library on the
  device, a drawn curve editor for list settings.
The full Phase 1 scope:
- **Definition language v0** (parser, formatter, clear errors) and the evaluator (dependency
  graph, caching, list handling).
- **The settings window**, generated from the project's parameters: groups, sliders, toggles,
  dropdowns, point editors.
- **First nodes:**
  - shapes: `revolve`, `loft`, `extrude`, profiles;
  - curves: `offset`, `fillet`, `smooth`, `resample`, `seam`;
  - shape → curves: **`spiral_on`, `offset_rings`, `contours`**;
  - **toolpath + G-code** for P1P and Klipper pellet (EasyGCode start/end blocks and bead model,
    golden-tested against EasyGCode).
- **Project files:** save, open, presets, plus a project library on the device and
  export/import as files.
- **First templates:** EasyGCode's *spiral disc*, thread connector, spoon, bend stool and
  lampshade (golden) and *vessel* (from general steps) rebuilt as projects.

**Done when:** you open the vessel project on your phone, change settings, export G-code, and
it prints like EasyGCode's. (The spiral disc already does: same G-code as EasyGCode's.)

## Phase 2: The agent (M), then 2b: the Draw tab
*The Chat tab landed early (15 Oct): Claude writes and edits recipes in the app, with your own
key (see [devlog.md](devlog.md)). Still to do from this phase: skills, the command line in the
chat box, and a test set of real requests. 2b adds the 2D curve editors from
[creating-shapes.md](creating-shapes.md); the steps as components already exist (the Graph
tab).*

- **The chat panel:**
  - your API key, Claude Opus 5.5 by default, model as a setting;
  - the chat box doubles as a command line (`height 200`, `export gcode`), with no AI involved.
- **Tools:** read the project, edit the recipe (small diffs), set values, render views,
  inspect, check the print, **load skills**, ask you questions.
- **Skills library v1,** written from EasyGCode's know-how: vase-mode vessel, threaded
  connector, lamp-holder mount, bend-stool seat, spiral disc.
- **Every agent change is reviewable:** a diff with Accept/Undo. Locked settings are respected,
  and guesses are marked *assumed*.
- **A small test set of your real requests,** re-run whenever the prompt or tools change.

**Done when:** "make me an E27 lampshade with 9 waves, for the pellet printer" gives a working
project with good settings, and "a bit taller" is just a slider.

## Phase 3: Toolpath toolkit (M–L)
- **More of EasyGCode as nodes and skills,** each with golden tests:
  - brims (incl. mouse ears);
  - seams (staircase/zipper/spiral) and fillet bottoms;
  - overhang compensation, patterns (weave/spikes), attractor spacing;
  - threads (ISO, E14/E27), legs/hairpins, foam events.
- **New operations:** `isocurves`, `project` (non-planar patterns), **`unroll` / `flatten`**
  (lay shapes out flat), `fill`.
- **"Save as skill"** for your own recipes.
- **Print checks:** overhang angles, bead overlap, bed fit, flow limits. GPU preview for very
  large paths.

## Phase 4: Comfort (M)
- **Send to the printer:** Klipper/Moonraker directly. Bambu LAN later.
  - Probably through a small optional desktop app, because browsers restrict talking to local
    devices.
- **Optional:** sync the project library between devices.

## Later / when needed
- Graph positions saved in the project file (a `layout` block); clusters (your own steps made
  from a group of components); full data trees.
- Rhino `.3dm` import/export, SVG/DXF.
- Solids and SolidWorks-style features (booleans, fillets, STEP).
- 3MF export for slicers (STL works since 1b).
- Claude Desktop/MCP control.
- Multi-threaded engine (needs cross-origin-isolation headers; see research §9).
- App Store builds.

---

## How we'll work
- **Small branches, merged often.** The EasyGCode collaboration rules apply.
- **Each change comes with tests and a docs update.** A short entry in [devlog.md](devlog.md)
  says what landed and what's still open.
- **Spikes are throwaway.** They live in `spikes/` with their results.
