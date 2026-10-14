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
| 3 | License | **Private (all rights reserved).** Everything we build on is permissively licensed (MIT/Apache/BSD; MPL is fine), so being private limits nothing. We just keep their license notices. We can open-source later at any time. |
| 4 | Printers | **Bambu P1P (filament) and Klipper pellet**, with EasyGCode's proven start/end G-code. |
| 5 | AI | **Built into the app with your own Claude API key** (standalone; see [api-key.md](api-key.md)). Claude Desktop/MCP control is optional, much later. |

**Still open: hosting.**
- The repo is private, and GitHub only serves websites from private repos on a paid plan
  (GitHub Pro, about $4/month).
- **Options:**
  - (a) GitHub Pro: simplest, one setting.
  - (b) Make the repo public: free, but the code is visible.
  - (c) Cloudflare Pages: free and also allows multi-threading headers, a bit more setup.
- CI is already prepared for (a) or (b).

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

## Phase 1: Grasshopper-lite + projects (M–L) ← *next*
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
- **First template:** EasyGCode's *vessel* rebuilt as a project.

**Done when:** you open the vessel project on your phone, change settings, export G-code, and
it prints like EasyGCode's.

## Phase 2: The agent (M)
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
- Graph view becomes editable (drag wires, node palette).
- Rhino `.3dm` import/export, SVG/DXF.
- Solids and SolidWorks-style features.
- 3MF/STL export for slicers.
- Claude Desktop/MCP control.
- Multi-threaded engine (needs cross-origin-isolation headers; see research §9).
- App Store builds.

---

## How we'll work
- **Small branches, merged often.** The EasyGCode collaboration rules apply.
- **Each change comes with tests and a docs update.** A short entry in [devlog.md](devlog.md)
  says what landed and what's still open.
- **Spikes are throwaway.** They live in `spikes/` with their results.
