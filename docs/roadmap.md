# EasyCAD roadmap

**Rule:** every phase ends with something you can actually use on your phone. We don't start a
phase until the previous one is useful. No rocket ship, just one working step at a time.

Sizes are relative (S ≈ one focused session, M ≈ a few, L ≈ many), assuming most code is
written with Claude Code and reviewed by you.

---

## Phase 0: Groundwork (S)
- Rust workspace (`crates/`) + `web/` app shell in the EasyGCode look, as a PWA with a
  manifest and service worker.
- CI:
  - `cargo fmt`, `clippy` (no-panic rules), tests;
  - `wasm32` build with a **size gate**;
  - deploy `web/` + the WASM to **GitHub Pages**.
- `CLAUDE.md` conventions, a dev log, and a layering check.

**Done when:** the PWA opens on your iPhone and shows a three.js viewport with a test shape
computed by the Rust/WASM engine.

## Phase 1: "Grasshopper-lite" without AI (M–L)
- **Language v0:** parser, canonical formatter, readable errors.
- **Evaluator:** dependency graph, caching, list replication, preview/final quality.
- **~30 nodes:** numbers & lists, points & transforms, curves (incl. `offset`, `fillet`,
  `divide`), regions, `extrude`/`revolve`/`loft`/`cap`, solid booleans, `bbox`/`volume`.
- **Params panel:** auto-generated from the file, with groups and locks. Point parameters
  get viewport handles.
- **Code view** (simple editor) and a **read-only graph view**.
- **Save/open** `.easycad`, autosave, **export STL + 3MF**.
- Examples: twisted vase, attractor panel, box with lid.

**Done when:** on your phone you open the twisted vase, drag sliders with live preview, and
the exported 3MF prints.

## Phase 2: The agent (M)
- **Chat panel:**
  - your own Claude API key, stored on the device;
  - **Claude Opus 5.5** by default, with the model as a setting;
  - the chat box also acts as a command line (`height 200`, `export 3mf`).
- **Tools:** `get_state`, `edit_definition`, `set_params`, `render_views`, `inspect`,
  `node_docs`, `ask_user`.
- **System prompt** generated from the node registry, with prompt caching.
- **Every agent turn is a diff** with Accept/Undo. The engine enforces locked parameters.
  `@assumed` badges appear for guessed values.
- **Small eval set:** about 20 real requests (yours). They're re-run whenever the prompt or
  tools change; each design must evaluate, expose sensible parameters and pass simple
  geometric checks.

**Done when:** "make me a pen holder with a hexagon pattern, 9 cm tall, for my P1P" gives a
working definition with good sliders, and "a bit taller" never needs the AI.

## Phase 3: Print path, EasyGCode inside (M–L)
- **`easycad-print`:**
  - toolpath type, the bead model, G-code writer (Klipper pellet, Marlin/Bambu filament);
  - start/end blocks, `vase_spiral`, `wave`, `brim`;
  - time/material estimates and a speed-coloured preview.
- **Golden tests against EasyGCode:** same settings give the same G-code (ignoring the
  timestamp line).
- **Port 2–3 EasyGCode projects** as `.easycad` templates (vessel, thread, lampshade).
- **Printability checks for solids:** overhang map, thin walls, bed fit, watertightness.

**Done when:** an EasyGCode vessel *and* a brand-new agent-made toolpath design both print
from the phone.

## Phase 4: Desktop app + MCP (M)
- **Tauri 2** desktop app (same UI, native engine, real files, API key in the OS keychain);
  installers on GitHub Releases.
- **`easycad mcp`:** Claude Desktop / Claude Code can drive the open document while you watch.
- **Printers:** send to Klipper/Moonraker; slice 3MF headless with OrcaSlicer's CLI.

**Done when:** you install a few-MB app on Mac/Windows, and Claude Code edits the model live in
front of you.

## Phase 5: Visual programming (M–L)
- **Editable node canvas:** drag wires, add nodes from a palette, rename. Every graph edit
  becomes a text edit.
- **Code editor:** autocomplete, inline errors, hover docs.
- **History timeline:** variants ("small", "large"), compare A/B.

## Phase 6: More Rhino (ongoing)
- **Interop:** `.3dm` import/export (rhino3dm.js), SVG/DXF import/export.
- **Curves and surfaces:** sweeps with two rails, trimmed surfaces, text/fonts as curves.
- **Fields and lattices:** distance-field nodes (gyroid, smooth blends) via `fidget` or
  manifold's SDF meshing.
- **Printing:** non-planar toolpaths.

## Phase 7: More SolidWorks (ongoing)
- **Sketching:** a 2D constraint sketcher (dimensions, coincident, tangent, …) with our own
  solver, as CADCraft did.
- **Solid features:** rounds and fillets on solids (distance-field first), shells, patterns.
- **Assemblies:** typed mates (Procedura-style), exploded views.
- **Exact geometry:** STEP export (exact B-rep via `truck`, or an optional OpenCascade plugin
  on desktop).
- **Drawings:** SVG/PDF with dimensions.

## Later / optional
- App Store / Play Store builds via Tauri 2 mobile
- WebMCP (browser agents)
- A shared gallery of definitions
- Plugin nodes compiled to WASM

---

## Decisions needed now

These shape Phase 0–1. My recommendation is first in each row.

| # | Question | Recommendation | Alternative |
|---|---|---|---|
| 1 | **UI stack** | **Rust engine (WASM) + plain web UI (three.js), PWA on phone, Tauri on desktop** | Pure-Rust UI (egui) like ArtCraft: great on desktop, weak on phones |
| 2 | **Document format** | **Own small dataflow language (`.easycad`)**, as drafted | JavaScript code-CAD (no graph view, unsafe) or OpenSCAD-compatible (no curves/NURBS) |
| 3 | **License** | **MIT OR Apache-2.0** (same as ArtCraft and our dependencies) | Keep private / unlicensed for now. EasyGCode currently has no license file. |
| 4 | **First printers** | **Bambu P1P (filament) + Klipper pellet**, as EasyGCode | Others? |
| 5 | **AI access first** | **In-app with your own API key** (works on the phone) | MCP first (desktop only, uses your Claude plan) |
| 6 | **Name / extension** | EasyCAD / `.easycad` | — |

---

## How we'll work

- **Small branches, merged often.** The EasyGCode collaboration rules apply: sync `main`
  before starting and before a PR.
- **Each change comes with tests and a docs update.** A dev log entry says what landed and
  what's still open, so the next session (human or agent) can continue.
- **Spikes are throwaway.** They live in `spikes/` with their results and are never imported
  by the app.
