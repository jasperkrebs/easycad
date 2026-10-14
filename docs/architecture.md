# EasyCAD architecture (proposal)

*Status: decisions answered on 14 Oct 2026 (see [roadmap.md](roadmap.md#decisions-answered-14-oct-2026)).
Phase 0 is in progress. The owner's priority is **custom toolpaths and G-code**; the printing
workflow is described in [toolpaths-and-skills.md](toolpaths-and-skills.md). Where this document
mentions solids/3MF/slicers, treat those as "later". Reasoning and sources are in
[research.md](research.md).*

---

## 1. The idea in one picture

```
  you ── "describe it" ──▶  agent ── writes / edits ──┐
                                                      ▼
  you ── sliders, handles, small edits ──▶  ┌──────────────────────────────┐
                                            │  definition  (vase.easycad)  │  ← the document
                                            │  parameters + named nodes    │
                                            └──────────────┬───────────────┘
                                                           │ evaluate (Rust engine, incremental)
                                                           ▼
                                   curves · surfaces · solids · toolpaths
                                                           │
                    ┌──────────────┬───────────────┬───────┴────────┬──────────────┐
                 viewport      3MF / STL        G-code         SVG / DXF      .3dm (Rhino)
```

Three rules hold everything together:

1. **The definition is the single source of truth.** It's a small text file that *is* a
   Grasshopper-style graph: parameters at the top, named nodes below. Geometry is never
   stored; it's always regenerated.
2. **Humans and the agent edit the definition through the same commands.** The agent is just
   a fast power user. Every change is a reviewable, undoable step.
3. **Tuning never needs the AI.** Sliders, handles, typing `height 200`, exporting and printing
   are instant, free, offline and deterministic. The AI is for *structure*: creating,
   restructuring, explaining, debugging.

---

## 2. Stack (decided 14 Oct 2026)

| Layer | Choice | Why |
|---|---|---|
| **Engine** | Rust workspace, compiled to **WebAssembly** (browser, phone) and natively (tests, CLI, optional desktop app) | One engine everywhere. Fast, safe, and very agent-friendly (the compiler catches mistakes). Same pattern as ArtCraft. |
| **Geometry** | `curvo` (NURBS curves/surfaces), `i_overlay` (2D regions/offsets), `kurbo` (2D paths/SVG); later `manifold-rust` (solids), `fidget` (fields) | Pure Rust, permissive licenses, **verified to compile to WASM: 0.44 MB gzipped, ~50 ms for a twisted-vase solve** ([spike](../spikes/kernel-probe/)). |
| **UI** | Plain HTML + CSS + JavaScript ES modules, no framework, **no bundler** (like EasyGCode), **three.js** viewport | Best possible phone experience (native inputs, keyboard, share sheet). Easy to style like EasyGCode. Zero build step for UI code. |
| **The app** | **A web app (PWA)**: the same app in the browser on computer, tablet and phone, installable from the browser, works offline | No installers, no app stores, updates arrive automatically. This is how EasyGCode already works. |
| **Desktop wrapper** | Optional, later: **Tauri 2** around the same `web/` UI | Only for what browsers can't do well: talking to printers on the local network, real file folders. |
| **Agent** | Claude API from inside the app with **your own key** (decided). MCP for Claude Desktop/Code is optional and much later | Standalone: no backend, no account. |

### 2.1 Lessons from browser games (why browser-first is the right call)
Today's browser games use exactly the ingredients this plan uses. Details in
[research §9](research.md#9-lessons-from-browser-games).

| Game technique | What EasyCAD does with it |
|---|---|
| Heavy code compiled to **WebAssembly** | The Rust engine (geometry, toolpaths, G-code) runs as WASM: near-native speed on the phone. |
| **GPU rendering** (WebGL2 → WebGPU) | three.js draws on the GPU. We start with WebGL2 (works on every phone), and the three.js `WebGPURenderer` (with automatic WebGL2 fallback) arrives when we need GPU compute or multi-million-segment toolpaths. |
| **Keep the main thread free** (workers) | The engine runs in a Web Worker; the UI and 3D view never stutter. Only the newest request counts (latest wins), as games drop stale frames. |
| **Multi-threading** (shared memory) | Later: needs "cross-origin isolation" headers. GitHub Pages can't set them (a known service-worker workaround exists); Cloudflare Pages can. |
| **Stream and lazy-load assets** | The core engine loads first (small). Heavy extras (Rhino file support, fonts, big skill libraries) load only when used. |
| **Local storage for big data** (OPFS) | The project library lives on the device (Origin Private File System / IndexedDB), with export/import as files. |
| **Frame budget thinking** | A slider drag must update the preview within ~1–2 frames: coarse preview while dragging, full quality on release. |
| **AI-assisted porting with verification** | The viral browser ports translate old code and check it against the original. We port EasyGCode's math the same way: translate, then golden-test against EasyGCode's own output. |

**Alternatives considered:**
- **Pure Rust UI with egui** (ArtCraft/Halfspace style). Great on desktop, but weak on phones:
  mobile text input, no official iOS target, multitouch not first-class.
- **Dioxus** (Rust compiled to HTML). Young, and UI iterations are slow.
- **Native Swift/Kotlin.** Three codebases.

Because the engine never depends on the UI (§4), the UI could be swapped later without
touching geometry.

---

## 3. Repository layout (target)

```
easycad/
├─ Cargo.toml              Rust workspace (edition 2024)
├─ crates/
│  ├─ lang/    easycad-lang     parser, formatter, syntax tree, error messages (no geometry)
│  ├─ geom/    easycad-geom     value types + thin wrappers over curvo / manifold-rust / i_overlay
│  ├─ nodes/   easycad-nodes    the node library: typed inputs/outputs, docs, examples
│  ├─ eval/    easycad-eval     graph building, type checking, incremental evaluation, caching
│  ├─ io/      easycad-io       3MF, STL, OBJ, SVG, DXF writers; importers
│  ├─ print/   easycad-print    toolpaths, bead model, G-code, printer profiles, printability checks
│  ├─ engine/  easycad-engine   session, command registry, history/undo, events
│  ├─ wasm/    easycad-wasm     wasm-bindgen facade used by the web UI (runs in a Web Worker)
│  ├─ cli/     easycad-cli      headless: eval, export, render, mcp
│  └─ mcp/     easycad-mcp      MCP server over the command registry (desktop)
├─ web/                    the app UI: static files, no build step for UI code
│  ├─ index.html  styles.css  manifest.webmanifest  sw.js
│  ├─ js/        app.js  engine.js  viewport.js  params.js  chat.js  graph.js  code.js  print.js
│  ├─ vendor/    three.module.js, anthropic-sdk.mjs, …   (pinned versions, committed)
│  └─ pkg/       generated by CI: easycad_wasm.js + easycad_wasm_bg.wasm
├─ desktop/                Tauri 2 shell (Phase 4)
├─ examples/               vase.easycad, box-with-lid.easycad, attractor-panel.easycad, …
├─ docs/
└─ spikes/                 throwaway experiments with results
```

**Start small:** Phase 1 can begin with just three crates (`lang`, a combined
`core` = geom + nodes + eval, and `wasm`) plus `web/`. They get split once they grow.

---

## 4. Layering rules (enforced by a check script, as ArtCraft does)

```
L0  lang            geom                     ← no UI, no I/O, no JS, compiles to wasm32
L1  nodes  → geom
L2  eval   → lang, nodes        io → geom        print → geom
L3  engine → eval, io, print
L4  wasm, cli, mcp, desktop → engine
UI  web/ talks ONLY to the engine's command API (via wasm in a worker, or Tauri IPC)
```

- Nothing below L4 knows about the browser, Tauri, the file system or the network.
- Everything up to L3 must compile to `wasm32-unknown-unknown` (checked in CI).

---

## 5. The document

### 5.1 The definition file (`.easycad`)
A small, readable, diffable text format. Full draft in
[definition-language.md](definition-language.md). A taste:

```
easycad 0.1
# Twisted star vase

param height    = 160 mm  in 60..300   "Height"
param radius    = 45 mm   in 15..120   "Base radius"
param points    = 7       in 3..16     "Star points"
param twist     = 90 deg  in 0..360    "Twist"
param top_scale = 0.8     in 0.3..1.5  "Top scale"

base     = star(points, outer: radius, inner: radius * 0.82) |> smooth(0.5)
t        = range(0, 1, count: 32)
sections = base |> scale(lerp(1, top_scale, t)) |> rotate(twist * t) |> move(z: height * t)
vase     = loft(sections) |> cap()        # close both ends: a printable solid

show vase
```

- **No loops, no mutation, no file or network access.** Each line is one node and each name
  is a wire. That's why the graph view is a 1:1 projection of the text, and why AI-written
  definitions are safe to run.
- **Lists replicate automatically** (Grasshopper/Dynamo style). `t` is a list of 32 numbers, so
  `sections` becomes a list of 32 curves without any loop.

### 5.2 Value types
`Number` (with a unit dimension: length, angle, ratio, count), `Bool`, `Text`, `Domain`
(`a..b`), `Point`, `Vector`, `Plane`, `Curve` (NURBS or polyline, 2D or 3D), `Region`
(planar areas with holes), `Surface`, `Solid` (watertight mesh), `Mesh` (display/reference),
`Toolpath`, `Printer`, `Record` (multi-output nodes, e.g. `bbox(x).max.z`), `List`.
Later: `Field` (distance fields).

### 5.3 Units and tolerances
- Internally everything is **mm** and **degrees**. Literals take units (`5 cm`, `2 in`,
  `90 deg`, `15 %`).
- **Document tolerances** (like Rhino's): model tolerance (default 0.001 mm), plus *preview*
  and *export* tessellation quality. The spike showed tessellation tolerance can swing cost by
  100×.
- **Fit tolerances are ordinary parameters** (`clearance`, `hole_compensation`), visible and
  tunable. The agent is told to create them instead of baking numbers in.
- **Printer profile** (nozzle, line width, layer height, bed size, XY compensation, max
  overhang) is a value that definitions can read, so parts adapt to the printer.

### 5.4 Lists (the Grasshopper part)
- Each node input is *item* or *list* access.
- An item input that receives a list makes the node run per item. With several lists, the
  **longest list** rule applies (Grasshopper's default).
- `cross(a, b)` gives every combination; `flatten`, `zip` and `shift` cover the common
  data-tree needs.
- Full Grasshopper data trees are deliberately **not** in v0. Nested lists cover most cases;
  revisit later.

---

## 6. Evaluation engine

1. **Parse** to a syntax tree, keeping comments and formatting. A canonical formatter means
   agent output and human edits always look the same.
2. **Resolve** names into a DAG (directed acyclic graph). Cycles and unknown names are errors
   with line numbers and suggestions ("did you mean `radius`?").
3. **Type check** against node signatures. Mistakes become readable errors, not crashes.
4. **Evaluate in topological order with caching.** Each node's result is keyed by its own text
   plus its inputs' hashes. Moving one slider re-runs only the nodes downstream of it
   (Grasshopper's "expire" model).
5. **Two quality levels.** *Preview* while dragging (coarse tessellation), *final* on release
   and export.
6. **Never block the UI.** The engine runs in a Web Worker (browser) or a thread pool
   (desktop). Meshes cross over as typed arrays.
7. **Errors stay local.** A failing node turns red with a message; independent branches still
   evaluate (like Grasshopper's orange/red components).
8. **Determinism and limits.** Same text + same parameters gives the same output (randomness
   is always seeded). Hard limits on list sizes, triangle counts and time turn a runaway
   definition into an error instead of a frozen phone.

---

## 7. Commands: one API for UI, agent, CLI and MCP

Borrowed directly from ArtCraft: **every action is a command** with a stable id, a JSON
parameter schema, docs and an undo record. The UI, the in-app agent, the CLI and the MCP
server all call the same registry.

| Area | Commands (initial) |
|---|---|
| Document | `doc.new`, `doc.open`, `doc.save`, `doc.export {format}` |
| Definition | `def.get`, `def.apply_edits {edits}`, `def.replace {text}`, `def.format` |
| Parameters | `param.set {name, value}`, `param.lock {name}`, `param.reset {name}` |
| Graph | `node.add`, `node.connect`, `node.remove`, `node.rename` (implemented as text edits) |
| Evaluation | `eval.run`, `eval.inspect {node}`, `eval.diagnostics` |
| View | `view.render {views, size}` (provided by the UI, returns PNGs) |
| Print | `print.check`, `print.toolpath`, `print.gcode {printer}`, `print.send {printer}` (desktop) |
| History | `history.undo`, `history.redo`, `history.snapshot {name}`, `history.list` |

**Undo** is a snapshot of (definition text, parameter values). Documents are tiny, so history
is cheap and can be kept forever. That also enables *variants* ("save as variant B").

---

## 8. The agent

### 8.1 Where it runs
*Decided 14 Oct 2026: option 1 (in the app, your own key) is the way. Options 2–3 are optional
and much later. Getting a key: [api-key.md](api-key.md). Agent "skills":
[toolpaths-and-skills.md](toolpaths-and-skills.md#skills-how-the-agent-learns-your-ways).*

1. **In the app** (phone, web, desktop):
   - A chat panel calls the Claude API directly with **your own API key**, stored only on the
     device (OS keychain on desktop). No server, no account.
   - The official TypeScript SDK supports this (`dangerouslyAllowBrowser: true`), which is
     acceptable because it's your key on your device.
   - The SDK is vendored as a pinned ES module.
2. **Through MCP** (desktop):
   - `easycad mcp` exposes the same tools to **Claude Desktop / Claude Code**, so you can use
     your Claude plan instead of API billing.
   - You watch the model update live in the app while Claude works.
3. **Later, WebMCP**, once browsers ship it unflagged: the PWA registers its commands as
   browser-agent tools.

### 8.2 Model (decide)
- **Default: Claude Opus 5.5**, adaptive thinking, effort `medium`
  (raise to `high` for "design something new").
- The model is a **user setting**: Sonnet 5.5 and Haiku 5.5 are cheaper options.
- Opus 5.5 can't force a specific tool, so tools use `tool_choice: auto` with `strict: true`
  schemas.

### 8.3 Tools (all are commands from §7)

| Tool | Purpose |
|---|---|
| `get_state` | Definition text with line numbers, parameter values/ranges/locks, diagnostics, scene summary (bounding boxes, volumes, counts) |
| `edit_definition` | Apply small find/replace edits (or a full text for new designs). The engine formats, evaluates and returns diagnostics plus what changed |
| `set_params` | Try values (e.g. check extremes of a range) |
| `render_views` | PNG renders from named views (iso/front/top/right), optionally highlighting a node. The agent's eyes |
| `inspect` | Type, size, volume, warnings of one node |
| `check_print` | Overhangs, thin walls, bed fit, watertightness |
| `node_docs` | Detailed docs + examples for specific nodes (keeps the base prompt small) |
| `ask_user` | A clarifying question with quick-reply chips. Ends the turn |

### 8.4 The loop
```
understand the request
  └─ critical dimension missing or conflicting?  → ask_user   (or guess AND mark it @assumed)
write / edit the definition  →  evaluate  →  fix errors (one at a time)
render 3–4 views  →  compare against the request  →  one targeted fix per round
finish: what changed · which parameters to play with · what was assumed
```
This follows the 2026 research: ask before inventing (ProCAD), check every step, use a visual
critic, small diffs (Procedura).

### 8.5 The system prompt (cached)
1. Role and collaboration rules (below).
2. **Language reference + node catalog, generated from the node registry** so it can never
   drift from the code. Roughly 10–20k tokens, cached so repeat calls are cheap.
3. Three to five example definitions.
4. Your printer profiles and preferences (units, default nozzle, your naming style).

### 8.6 Collaboration rules (enforced by the engine where possible)
- Expose every meaningful dimension as a `param` with a sensible range, a label and a group.
  Tolerances are always explicit parameters.
- **Locked parameters can't be changed by the agent.** The engine rejects the edit and says why.
- Minimal diffs; never silently rewrite your manual changes; whole-file rewrites only for
  brand-new designs.
- Guesses are visible: `param clearance = 0.3 mm in 0..1 "Lid clearance" @assumed`.
- **The chat box is also a command line.** `height 200`, `twist +15`, `export 3mf`, `undo`,
  `print` are handled locally, instantly and for free. Only real requests go to the AI.

### 8.7 Cost and privacy
- Tuning, exporting and printing cost nothing.
- **Rough estimate:** a design request with Opus 5.5 costs a few cents up to about $1,
  depending on how many render-and-fix rounds it takes. Model prices are $4 / $20 per
  million input/output tokens, and caching makes the large reference cheap.
- What gets sent: your message, the definition text, parameter values and rendered images.
  No files, no keys, nothing else.

---

## 9. User interface

### 9.1 Layouts

**Phone (portrait):**
```
┌──────────────────────────────┐
│ EasyCAD   Twisted vase  ↶ ↷ ⋯│
├──────────────────────────────┤
│                              │
│         3D viewport          │  drag = orbit, pinch = zoom, two fingers = pan
│                              │
├──────────────────────────────┤
│ [Params] [Graph] [Code] [Print]
│  Shape                       │
│  Height      ━━━━●━━━ 160 mm 🔒│
│  Twist       ━━●━━━━━  90°   │
│  …                           │
├──────────────────────────────┤
│ 💬 Describe a change… or "height 200" │
└──────────────────────────────┘
```

**Desktop (≥ 960 px):** viewport left (large). The right column holds tabs for Params / Graph /
Code / Print, with the chat docked below. The Graph view can split side-by-side with the
viewport, like Rhino + Grasshopper.

### 9.2 Panels
- **Viewport** (three.js):
  - shaded + edges, curves as fat lines, toolpaths coloured by speed (as in EasyGCode);
  - section plane, measure tool, x-ray;
  - **handles**: point parameters show as draggable dots, and transforms show a gizmo.
    Dragging writes parameter values, no AI involved.
- **Params:** auto-generated from `param` lines:
  - sliders, number fields, toggles and dropdowns, grouped;
  - lock icons, and "assumed" badges with a one-tap "OK, keep it".
- **Graph:** the definition drawn as nodes and wires, coloured by status. Read-only first,
  editable in Phase 5 (edits become text edits).
- **Code:** the `.easycad` text with highlighting and inline errors (a simple editor first,
  CodeMirror later).
- **Print:** printer, material, estimates (time, length, weight), checks, export buttons, send
  to printer (desktop).
- **Chat:** conversation, the change summary for each agent turn with **Accept / Undo**, and
  quick-reply chips for questions.

### 9.3 Direct modelling without AI (the "simple tasks" part)
- Tap or click to place points, giving a `polyline`/`interpolate` node whose points are an
  editable list parameter. This is the equivalent of Grasshopper's "set multiple points"
  (internalised data).
- Drag handles and use gizmos (as above).
- "Bake" a result into a fixed shape to stop it changing; "un-bake" by undo.

### 9.4 Look
- **Same design tokens as EasyGCode** for the dark theme: `--bg #0f1115`, `--card #181b22`,
  `--card2 #1f232c`, `--text #e7eaf0`, `--muted #9aa3b2`, `--accent #4f9dff`,
  `--accent2 #2bd9a0`, `--warn #ffb454`, `--border #2a2f3a`.
- Plus a matching light theme for daylight use, following the system setting.
- System font, rounded cards, a scrollable tab row, safe-area insets.
- See the [mockup](mockup/index.html).

---

## 10. Files and interop

**Project files** (decided 14 Oct 2026):
- One project is one `.easycad` file with project info, settings, the recipe, optional presets
  and the optional agent conversation.
- Opening it brings back the same settings window. Details in
  [toolpaths-and-skills.md](toolpaths-and-skills.md#project-files-save-and-reopen-exactly-where-you-were).
- Projects live in an on-device library (OPFS/IndexedDB) and can be exported/imported as
  files.

| Format | Direction | Notes |
|---|---|---|
| `.easycad` | open/save | The project (text, diffable, git-friendly). A trailing optional `layout` block stores graph-node positions. |
| **G-code** | export/send | **The main output** (§11): custom toolpaths for Bambu P1P and Klipper pellet. |
| Settings presets (`.json`) | export/import | Values only, like EasyGCode's "Save settings". |
| 3MF, STL, OBJ, GLB | export | Later, only when needed. |
| SVG, DXF | export/import | 2D curves for laser cutting, plotting, references. |
| **`.3dm` (Rhino)** | export/import | Via **rhino3dm.js** (MIT), loaded only when used. Curves and NURBS surfaces export as real NURBS; meshes as meshes. Import brings curves in as reference parameters. |
| STL/3MF | import | As reference meshes or boolean inputs (manifold-rust's robust engine repairs messy meshes). |
| STEP | later | Needs exact B-rep (`truck`, or an optional OpenCascade plugin on desktop). |

---

## 11. 3D printing

**Decided 14 Oct 2026: custom toolpaths come first** (§11.2, and the full workflow in
[toolpaths-and-skills.md](toolpaths-and-skills.md)). No slicing logic, no slicer round-trips.
The solid path below stays as a later option.

### 11.1 Solid path (later)
Definition → watertight solid → **printability checks**:
- overhang map against the printer's max angle;
- walls thinner than two line widths;
- bed fit;
- open edges.

Then **3MF** to Bambu Studio / OrcaSlicer / PrusaSlicer. On desktop, optionally slice headless
with **OrcaSlicer's CLI** and send.

### 11.2 Toolpath path (EasyGCode inside EasyCAD)
Curves are toolpaths:
- `vase_spiral`, `toolpath(curve, width, height)`, patterns (waves, spikes),
  fillets/overhang compensation, brims;
- the volumetric bead model `beadArea(w,h) = (w−h)·h + π·(h/2)²`;
- pellet (Klipper) and filament (Marlin/Bambu) modes.

The preview colours toolpaths by feed, exactly as EasyGCode does.

**Migration plan for EasyGCode's engine:**
1. Port the shared core (bead model, G-code writer, start/end blocks, vase spiral, brim) to
   `easycad-print`.
2. **Golden tests:** for the same settings, EasyCAD's G-code must match EasyGCode's
   byte-for-byte (ignoring the timestamp line), using EasyGCode's own regression method.
3. Re-express each EasyGCode project (vessel, lampshade, thread, container, spiral disc, coat
   hanger, …) as an `.easycad` template. After that the agent can make new ones ("a lampshade
   for E27 with a wavy rim").
4. EasyGCode keeps working as-is the whole time. Nothing is taken away.

### 11.3 Sending to printers
- **Klipper/Moonraker:** from the desktop app (reliable). From the web app it only works if
  the browser allows local-network requests and Moonraker's `cors_domains` lists the app.
  Chrome yes (with a permission prompt); iPhone Safari probably not.
- **Bambu:** LAN + Developer Mode only, desktop app, later.
- **Always works:** download / iOS share sheet, as EasyGCode does now.

---

## 12. Distribution

| Target | How |
|---|---|
| **Web / phone** (the app) | GitHub Actions builds the WASM, then deploys `web/` to static hosting. Installable PWA, works offline. **The repo is private:** GitHub Pages then needs GitHub Pro. Alternatives are a public repo or Cloudflare Pages (free, and can send the headers needed for multi-threading). The deploy job is ready and switched on by a repo variable. |
| **Desktop** | Tauri 2 builds macOS (`.dmg`), Windows (`.msi`/`.exe`) and Linux (AppImage/`.deb`) installers on GitHub Releases. A few MB each. Code signing (Apple Developer account for macOS notarisation; optional Windows signing) avoids "unknown developer" warnings. |
| **App Store / Play Store** | Optional, later, via Tauri 2 mobile from the same code. Needs Xcode on a Mac and an Apple Developer account to build and sign, but no Swift code. The PWA covers the phone until then. |

---

## 13. Engineering rules

- **Never crash.** No `unwrap`/`expect`/`panic` in non-test Rust (clippy deny list), no
  `unsafe`. A bad definition, file or parameter gives an error message, never a crash.
- **Never break WASM.** CI builds `wasm32` on every change. **Size budget:** ≤ 3 MB gzipped
  for the engine to start with (the spike is at 0.44 MB).
- **Performance budget:** a parameter change gives a preview in under 100 ms on a mid-range
  phone for the example definitions.
- **Tests with every change:**
  - unit tests;
  - golden tests (definition → mesh volume/bounds/hash; settings → G-code text);
  - parser round-trip and fuzz tests;
  - a Playwright smoke test of the UI (Chromium is available in CI).
- **Agent-ready repo** (from ArtCraft): `CLAUDE.md` first, then docs, then a short dev log so
  every new session can pick up where the last one stopped.
- **Small, boring dependencies.** Pinned, permissive licenses only (MIT/Apache/BSD/0BSD;
  MPL-2.0 acceptable; no GPL/AGPL in the shipped app).
  - EasyCAD itself is **private (all rights reserved)**, decided 14 Oct 2026. Permissive licenses
    allow that.
  - Their notices are listed in `THIRD-PARTY-NOTICES.md` and shipped with the app.

---

## 14. Risks and how we handle them

| Risk | Mitigation |
|---|---|
| The language design is the biggest decision | Keep v0 tiny. Version header in every file (`easycad 0.1`). Canonical formatter. Automatic migration when the syntax changes. |
| `curvo` and `manifold-rust` are 0.x / new | Wrap them behind our own `geom` API, cover with golden tests, pin versions. Fallback for booleans: the C++ Manifold via its official WASM package. |
| Mesh-only solids can't do true edge fillets or STEP yet | Fillet in the profiles (curve fillets) and round with distance fields for now. Exact B-rep is a later milestone (Phase 7). |
| iOS can evict PWA storage | Explicit save/share of `.easycad` files, autosave plus "export backup", the desktop app. |
| AI quality varies | AI is optional; locks, diffs, undo; a small eval set of real prompts run on every prompt change. |
| Scope creep ("rocket ship") | The roadmap phases each end with something usable on the phone. Nothing starts before the previous phase is useful. |
