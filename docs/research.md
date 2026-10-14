# EasyCAD research: what exists, what doesn't, what to build on

*Research date: 14 October 2026. Everything below was checked against current sources
(linked at the end) or measured in this repo. Things that couldn't be verified are marked.*

---

## 1. The short version

1. **The gap is real.** Nothing open, lightweight and local works like *Grasshopper with an
   AI partner*: you describe a thing, an agent builds a parametric definition, and you tune it
   with sliders. Existing AI CAD tools fall into one of these groups:
   - cloud-bound (Zoo),
   - mechanical/feature-tree oriented (Zoo, Onshape, Fusion),
   - limited to OpenSCAD's CSG world (CADAM, MakerWorld),
   - mesh generators you can't edit (Meshy, Tripo),
   - bolted onto heavy desktop tools through MCP (Rhino/Grasshopper MCP servers),
   - closed desktop products (Formas.AI Cartesian, rolling out from 31 Oct 2026).
2. **"ArtCraft" is the Rust Adobe-rewrite you heard about.** It's a set of Rust apps
   (PhotoCraft, VectorCraft, CADCraft, …) that went public on 30 Sept 2026, written largely
   with Claude. Its **CADCraft is an AutoCAD-style 2D drafting app with no 3D at all**, so there
   is no "Rhino/Grasshopper-craft". Their architecture lessons are excellent and worth copying
   (§3.4).
3. **The geometry building blocks now exist in pure Rust.** Two libraries matter:
   - **`curvo`**: NURBS curves and surfaces, i.e. Rhino-style curves, loft, sweep, revolve,
     offset and fillet.
   - **`manifold-rust`**: watertight solids and robust booleans, a complete port of the Manifold
     library that OpenSCAD and Blender use. It was first released in July 2026.

   I compiled both to WebAssembly in this repo. The bundle is **0.44 MB gzipped**, and a
   twisted-vase solve takes **~50 ms in WASM** (§4.2). A year ago this wasn't possible
   without C++.
4. **For the AI, a small functional language beats JSON and beats "real" code.** The newest
   benchmark (P3D-Bench, June 2026) found OpenSCAD-style compact functional code the most
   reliable format. Python CadQuery more often fails to run, and JSON is weakest. The newest
   agentic-CAD papers (Procedura, CADIR, Aug 2026) converge on *named parts + explicit
   dependency graphs + a visual critic*. That shape is literally a Grasshopper definition
   written as text.
5. **Agents should ask before they invent dimensions.** ProCAD (Feb 2026) cut geometric error
   by ~80% just by letting a clarifying agent ask about missing or conflicting dimensions.
6. **Phones can run it.** WebGPU ships in Safari on iOS 26 (and WebGL2 works everywhere).
   But the pure-Rust UI toolkits ArtCraft and Halfspace use (egui) are still weak on phones:
   - text input on mobile web,
   - no official iOS target,
   - multitouch is an afterthought.

   A **Rust engine + thin web UI** keeps the EasyGCode phone experience.
7. **3D printing integration can stay cheap.** Use 3MF as the bridge to Bambu Studio / Orca /
   Prusa, OrcaSlicer's CLI on desktop, and your own EasyGCode toolpath engine for "print the
   curve directly". Writing a full slicer is not needed.

---

## 2. Your starting point: EasyGCode

I read the whole repo (`jasperkrebstc/easygcode`, ~15.5k lines):

- **What it is:** a phone-first, install-free PWA that generates G-code *directly from
  parametric curves*. Project tabs: coat hanger, bend stool, vessel, spoon, lampshade,
  container, thread, spiral disc.
- **Tech:** plain vanilla JS, no build step, no npm. `geometry.js` holds shapes, offsets,
  samplers and fills. `gcode.js` holds the bead model, spirals, brims and printer modes.
  `app.js` holds the UI and a hand-written 2D-canvas 3D preview. A service worker makes it
  work offline. It's hosted on GitHub Pages.
- **Style:** dark cards (`#0f1115` background, `#181b22` cards, `#4f9dff` accent, `#2bd9a0`
  second accent), 14 px radii, system font, max-width 640 px, scrollable tab row,
  one-field-at-a-time editors for curve points, "randomize" with seeds.
- **What it already proves:**
  - parametric definitions + direct toolpaths are a great fit for pellet and vase-mode printing;
  - a phone-only workflow is genuinely usable;
  - verified geometric reasoning (thread fits from ISO 68-1, overhang compensation) is
    where the value is.
- **What it tells us EasyCAD needs:**
  - Each EasyGCode project is a *hand-coded parametric definition*. EasyCAD makes those
    definitions data that a human or an agent can create, so new "projects" no longer need
    new code.
  - The README roadmap already says "AI text-to-shape". EasyCAD is that idea generalised.
  - Its toolpath math (bead area, volumetric E, vase spiral, fillets, brims, Klipper/Marlin
    start/end) should be **ported, not re-invented**, with golden tests against EasyGCode's
    output (it already has a byte-identical regression habit).

---

## 3. What exists today

### 3.1 AI-first CAD products

| Product | What it does | Approach | Takeaway for us |
|---|---|---|---|
| **Zoo Design Studio** (+ Text-to-CAD, Zookeeper agent) | Mechanical CAD with point-and-click, a code language (KCL) and prompting | Own kernel and own language. **The 3D view is a video stream from Zoo's cloud GPU engine** over WebSockets | Code-as-source-of-truth works and gives editability. Cloud-bound means no offline, no phone-local, and per-second pricing. Don't do that. |
| **CADAM** (Adam, YC; open source, GPL-3.0) | Browser text-to-CAD | LLM writes OpenSCAD → OpenSCAD-WASM compiles → Three.js preview. **Dimensions are pulled out as sliders; slider tweaks bypass the AI entirely** | The "agent builds, human tunes" loop is proven and popular (thousands of GitHub stars). OpenSCAD limits it to CSG solids: no curves, no NURBS, no toolpaths. |
| **Formas.AI Cartesian** (announced Oct 2026) | Agentic modeller for architecture, interiors, furniture, product | Proprietary agent-oriented kernel; orchestrates several models (incl. Claude Opus 5.5). Constraints for thickness, spacing and assemblies. STEP/STL/DXF/IFC; live Rhino and SketchUp links. Desktop (macOS/Windows) only | Closest in spirit to your idea, and it validates the direction. Closed, desktop-only, not print-focused. |
| **Autodesk (AU 2026)** | "Agentic ecosystems" across its clouds | Preview; pricing and licensing not until 2027 | Incumbents are moving, but slowly and inside heavy suites. |
| **Onshape AI Advisor, Leo AI, MecAgent, …** | Copilots that answer questions or assist inside existing CAD | Docs-grounded assistants | They help with the old tools; they don't change the modelling paradigm. |
| **Meshy, Tripo, Rodin, Hunyuan3D, …** | Text/image → mesh | Generative meshes | Fine for figurines. Not dimensionally controllable or editable; wrong tool for functional prints. |
| **MakerWorld Parametric Model Maker** (Bambu) | OpenSCAD customizer on MakerWorld; export STL/3MF | Server-side OpenSCAD with a form UI | Shows real demand for "tweak the parameters, then print". |

A 2026 industry overview sums it up:
- Group the tools as **copilots**, **modelling agents**, **text-to-CAD** and **generative design**.
- **Test revisions, not first drafts.** Editability and the hand-off decide usefulness.
- Successful execution doesn't prove the model is good.

### 3.2 Agents bolted onto Rhino / Grasshopper

There are several community MCP servers:
- **RhinoMCP** forks: they let Claude run Python inside Rhino.
- **Cordyceps**: a C# Grasshopper component for Rhino 8.21+. It adds components, wires them,
  sets values, groups, bakes and renders.
- **Grasshopper Parametric Design MCP**: a Python bridge.
- The **Raven** plugin.

I found **no official McNeel MCP server**. **Grasshopper 2** is in beta inside Rhino 9 and not
production-ready (GH1 plugins don't load in it).

**Takeaway:** people want exactly this, an agent wiring a Grasshopper definition. Today it
means running a commercial desktop app plus a fragile Python/C# bridge, on desktop only. EasyCAD
can be the native version and still talk to Rhino through `.3dm` files (§6.4).

### 3.3 Code-CAD and node-based tools

| Tool | Notes |
|---|---|
| **OpenSCAD** (+ Manifold backend) | Small pure-functional CSG language. LLMs write it reliably (§3.5). No curves or NURBS; global `$fn` resolution. |
| **CadQuery / build123d** (Python, OpenCascade) | B-rep with fillets and STEP. Heavy (OCCT), and LLM-written code fails to run more often. |
| **replicad, JSCAD, ManifoldCAD** | JS code-CAD in the browser (OCCT-WASM or Manifold-WASM). Imperative code, so it can't be shown as a node graph. |
| **Dynamo DesignScript** (Autodesk) | Key precedent. **Every Dynamo node graph is serialized to and executed as DesignScript text**, with "replication" rules for lists (like Grasshopper's list matching). A textual dataflow language *is* a node graph. |
| **Graphite** (Rust) | Node-based procedural 2D editor; every edit is a node; layer view and node view mirror each other. Desktop app reached a release candidate in Jan 2026 (moved off Electron to native). Shows the "graph underneath, simple UI on top" model works. |
| **Halfspace** (Matt Keeter, Rust) | Distance-field CAD IDE: Rust + egui + wgpu/WebGPU, **Rhai** scripts, the Fidget kernel; desktop and browser from one codebase. His notes: mobile Safari WebGPU issues, and pan/zoom not designed for multitouch. |
| **Plasticity** | The modern "artist-friendly NURBS modeller" everyone likes. Built on the commercial **Parasolid** kernel, which isn't an option for us. |
| **FullControl GCode Designer** (Loughborough Univ.) | Design every toolpath segment directly, without slicing; non-planar freedom. Same philosophy as EasyGCode's direct curve printing. |

### 3.4 The ArtCraft wave, and what to copy from it

ArtCraft (storytold, Brandon Thomas) is a family of "clean-room" Rust rewrites of Adobe and
Autodesk tools. The first repos went public on 30 Sept 2026, built largely with Claude.
PhotoCraft's own docs count ~539k lines of Rust and ~8k tests as of 13 Oct, against a design
plan dated 30 Sept. Critics question the code quality and the AI-driven clean-room claim, but
the **architecture is genuinely good and directly reusable as a pattern**. I read PhotoCraft's and CADCraft's
`AGENTS.md` and architecture docs:

- **Everything is a command.** Every menu item, tool and dialog is one command with a stable
  id and typed JSON params, in one registry. The **UI, CLI, a JSON control channel and an
  MCP server** all dispatch the same commands. The UI is "a thin shell". An agent can do
  anything a human can, and vice versa.
- **Headless first.** Every feature works from tests, the CLI and MCP without a GUI.
- **Enforced layering.** Crates may only depend on lower layers (`cargo xtask layers`).
  Nothing below the UI crate may touch the UI toolkit, so **the UI is swappable**.
- **Never crash.** No `unwrap`/`panic` in non-test code (clippy-enforced), no `unsafe`,
  input-derived numbers are treated as hostile, and a fuzz test throws adversarial params at
  every command.
- **Never break WASM.** Lower crates must compile to `wasm32`. The web build has a hard size
  gate (24 MiB for PhotoCraft).
- **Agent-ready repo.** `AGENTS.md` comes first, then architecture → gaps → roadmap, plus a dev
  log so the next agent session can pick up. Parity and score docs say what to do next.
- **UI stack:** egui/eframe 0.36 on wgpu natively, and the same Rust compiled to WASM for the
  browser. Explicitly no Tauri/Electron/JS.
- **CADCraft specifically:**
  - AutoCAD-style 2D drafting (command line, snaps, layers, dimensions, hatches, blocks, DXF).
  - Its own geometry crate and its own constraint solver (damped Gauss-Newton).
  - It refuses all GPL/LGPL code, OpenCASCADE included.
  - Its own gap list says: **"No 3D: UCS, 3D views, orbit, solids, surfaces, meshes …
    (150 menu items)"**.

What we should **not** copy: the goal of 1:1 parity with an old product, and egui as the
phone UI (§5).

### 3.5 Research on AI + CAD (2026)

| Paper | Finding | Design consequence |
|---|---|---|
| **P3D-Bench** (Jun 2026). Compares JSON, OpenSCAD, CadQuery and Three.js outputs across 11 models | **"OpenSCAD is the strongest format"**: highest validity and geometry scores. CadQuery "often produces invalid programs that fail to run". Three.js output is mesh-like (lower topology score). Models get *semantics* right (~0.8) far more than *exact geometry* (~0.35). Multi-turn render feedback helped some models modestly. | Use a **small, constrained, functional language**, not JSON and not general Python/JS. Expect the human to fix exact dimensions, so make that trivial (sliders). |
| **Text2CAD-Bench** (May 2026) | Chose CadQuery because method chaining fits language and the code is executable/verifiable | Executable, checkable output matters; chaining (`a |> b |> c`) reads naturally. |
| **ProCAD: "Clarify Before You Draw"** (Feb 2026) | A clarifying agent asks only when critical dimensions are missing or conflicting. **−79.9% Chamfer distance vs Claude Sonnet 4.5; invalid outputs 4.8% → 0.9%** | The agent asks first, or makes the guess explicit as a parameter marked *assumed*. |
| **Procedura** (Aug 2026) | Agent writes a parametric program of **named parts joined by typed mates**. A part is admitted only after compile/mate/connectivity checks. A separate **vision critic** applies one diagnosed fix at a time. Beats prior 3D-code agents | Named nodes, checks after every edit, rendered views for self-review, small diffs instead of rewrites. Typed mates are the later "assemblies" feature. |
| **CADIR** (Aug 2026) | An explicit **construction graph** (operations, parameter dependencies, constraints, topology references) beats "sequential scripts with implicit dependencies" for editability across CAD systems | The document *is* the dependency graph. Dependencies must be explicit (named wires), not hidden in mutable state. |

---

## 4. Geometry kernels (what to build on)

### 4.1 Candidates (crates.io data, 14 Oct 2026)

| Need | Library | Version / activity | License | Verdict |
|---|---|---|---|---|
| NURBS curves & surfaces (the "Rhino" part) | **`curvo`** (mattatz) | 0.3.2, updated 7 Oct 2026 | MIT | **Use.** Pure Rust. Interpolation, periodic curves, offset, curve fillet, curve booleans (region union/difference, sponsored by VUILD), intersections, divide by length, closest point, Frenet frames, extrude/loft/sweep/revolve, trimmed surfaces, adaptive tessellation. |
| Watertight solids, booleans (the "printable" part) | **`manifold-rust`** (Lars Brubaker, MatterHackers) | 0.16.0, created Jul 2026, updated 3 Oct 2026 | Apache-2.0 | **Use.** Complete pure-Rust port of Manifold 3.5 (the library behind OpenSCAD's and Blender's fast booleans): booleans, extrude (twist/scale), revolve, hull, Minkowski, SDF level sets, smoothing, 2D cross-sections. Adds a **robust engine** that survived the whole Thingi10K corpus. 721+ ported tests. WASM demo. Used in production 3D-printing pipelines. |
| 2D polygons: booleans, offsets | **`i_overlay`** | 9.1.0, updated today | MIT/Apache | **Use** for sketch regions, slicing contours, brims and fills. |
| 2D Béziers / SVG paths | **`kurbo`** (linebender) | 0.13.1 | MIT/Apache | **Use** for SVG import/export and fonts later. |
| Implicit / distance fields (lattices, gyroids, smooth blends) | **`fidget`** (Matt Keeter) | 0.5.1, Sep 2026 | MPL-2.0 | **Later.** Interpreter on WASM, JIT natively; Manifold Dual Contouring meshing. MPL is fine to depend on. (`manifold-rust` also has basic SDF meshing.) |
| Alternative mesh booleans | `boolmesh` | 0.2.1 | MPL-2.0 | Backup option only. |
| B-rep + STEP | `truck` (RICOS) | crates last published Sep 2024 (repo active) | Apache-2.0 | **Watch.** Needed for STEP export of exact surfaces and for SolidWorks-style edge fillets later. |
| B-rep with fillets | `brepkit` | 4.1.x, very active | **AGPL-3.0** | **Avoid.** The license would force the whole app to AGPL. |
| OpenCascade wrapper | `opencascade` (opencascade-rs) | 0.3.0 | LGPL-2.1 | **Optional desktop-only plugin, much later.** Heavy C++; poor fit for WASM/phones. |
| Fornjot | — | Mainline no longer developed | — | Skip. |
| File formats | `threemf` (0BSD), `stl_io` (MIT) | current | permissive | Use. |
| MCP server | `rmcp` (official Rust MCP SDK) | 3.5.1 | Apache-2.0 | Use (desktop). |

**Why mesh-first solids are OK:** a 3D printer always receives a mesh (STL/3MF) or a toolpath.
We keep curves and surfaces exact (NURBS) for as long as possible and only turn them into
watertight meshes at the "solid" step. Exact B-rep (STEP export, rolling-ball edge fillets) is
a later, separate milestone. It isn't required for printing, and no permissively licensed
pure-Rust B-rep kernel is mature yet.

### 4.2 Measured: does it actually run on a phone-class target?

A throwaway probe lives in [`spikes/kernel-probe/`](../spikes/kernel-probe/). It builds a
Grasshopper-style **twisted star vase** two ways:

- **NURBS:** a smooth periodic 7-point star, 24 sections scaled, rotated and lifted, then
  lofted and adaptively tessellated (≈11k triangles). This is the curvo path.
- **Solid:** a twisted, scaled extrusion hollowed by a boolean difference, giving a watertight
  shell of 3,720 triangles and a 69,232 mm³ volume. This is the manifold-rust path.

| | Result |
|---|---|
| Compiles to `wasm32-unknown-unknown` unchanged | **Yes** (both crates) |
| WASM size (release, opt `s`, LTO, before `wasm-opt`) | **1.88 MB raw / 0.44 MB gzipped** |
| Native time, whole solve | **33 ms** |
| WASM time (Node 22), steady state | **~50 ms** (first call ~100 ms) |
| Tessellation sensitivity | Normal tolerance 0.001 → 754k triangles and ~2 s. 0.1 → 11k triangles and 33 ms. **Preview vs export resolution must be a first-class setting.** |

Measured on a cloud server CPU; expect phones to be roughly 2–4× slower (estimate). That's
still interactive for slider dragging, especially with incremental re-evaluation (§ Architecture).

---

## 5. App shell and platforms

| Option | Phone | Desktop install | One language | Notes |
|---|---|---|---|---|
| **A. Pure Rust UI (egui + wgpu)**, ArtCraft/Halfspace style | ⚠️ Web build only. eframe has no official iOS target. Mobile text input was poor until egui 0.36 (Aug 2026) and is still flagged. Multitouch not first-class | ✅ native | ✅ | Proven on desktop + web. Phone UX would feel like a desktop tool. Bigger WASM downloads. |
| **B. Rust engine (WASM) + thin web UI** (HTML/CSS/JS + three.js), **PWA** on the phone, **Tauri 2** for desktop installers | ✅ Native inputs and keyboards, share sheet, Add to Home Screen, offline: exactly EasyGCode's strengths | ✅ Tauri: small installers (a few MB, system webview), Rust runs natively inside | Rust + JS | Engine stays 100% Rust and headless-testable; only the UI is web. Tauri 2 can also build iOS/Android apps later (Xcode only to build/sign; no Swift to write). |
| C. Dioxus (Rust → DOM) | ✅ | ✅ | ✅ | Interesting, but younger ecosystem, slow UI iteration (Rust compile times), and the 3D viewport still has to be built by hand. |
| D. Native Swift/Kotlin apps | ✅ | ❌ | ❌ | Three codebases. No. |

Platform facts that matter:
- **WebGPU** is on by default in Safari on iOS/iPadOS/macOS 26 (since Sept 2025). WebGL2 is
  everywhere, so a three.js viewport runs on any phone today.
- **PWA on iPhone** works the way EasyGCode already does (Share → Add to Home Screen, offline
  via service worker). iOS may clear local storage of rarely used web apps, so files must also
  be exportable/shareable, as EasyGCode already handles.
- **Tauri 2** (2.12.4, released today) is production-ready on desktop. Mobile works but is
  younger.

**Recommendation: B** (details in [architecture.md](architecture.md)). It's "Rust where it
counts" (geometry, evaluation, export, G-code) with the UI tech that is best on phones.
Because the engine never depends on the UI (the ArtCraft layering rule), an egui desktop UI
could still be added later.

---

## 6. 3D printing integration

### 6.1 Two print paths
1. **Solid path** (most parts): definition → watertight mesh → printability checks →
   **3MF** (works in Bambu Studio, OrcaSlicer, PrusaSlicer; keeps units and multiple objects).
2. **Curve/toolpath path** (EasyGCode heritage, like FullControl): curves *are* the toolpath →
   G-code with the volumetric bead model. Covers vase mode, non-planar, pellet, and spikes,
   weaves and fillets.

### 6.2 Slicing without writing a slicer
- **OrcaSlicer CLI** slices a 3MF headless (`--slice`, `--load-settings`, `--export-3mf`).
  Usable from the desktop app.
- **Kiri:Moto** (Grid.Space, MIT) is a full slicer that runs entirely in the browser, so
  in-app slicing on the phone is possible later.
- A small built-in "spiralize" (slice a solid into one continuous vase-mode path) is easy in
  Rust with `i_overlay` and fits the EasyGCode feature set.

### 6.3 Sending to the printer
- **Klipper/Moonraker:** HTTP upload + start print. For a browser app the printer must allow
  the origin (`[authorization] cors_domains`). A page served over HTTPS (GitHub Pages) calling
  `http://printer.local` hits mixed-content rules:
  - **Chrome** now allows it after a "local network access" permission prompt.
  - **Safari (iPhone)** support is unclear; assume it's blocked.
  - **Desktop app (Tauri):** no browser restrictions, works reliably.
- **Bambu Lab:** third-party control needs LAN-only + **Developer Mode** (MQTT + FTP,
  not officially supported, cloud disabled). So it's desktop-app only, and later.
- **Phone fallback that always works:** the iOS share sheet / download, then open in Bambu
  Handy, Mainsail or Fluidd, exactly like EasyGCode today.

### 6.4 Interop with Rhino
**rhino3dm.js** (McNeel, MIT) reads and writes `.3dm` in the browser via WASM. The binary is
large, so load it only when needed. It gives EasyCAD a clean bridge to Rhino: send curves out
and pull reference curves in. That beats any live-link plugin for a first version.

---

## 7. AI integration facts (Oct 2026)

- **Models** (Anthropic API, per million tokens in/out):
  - **Claude Opus 5.5**: $4 / $20. Thinking is adaptive; effort defaults to
    `medium`.
  - Claude Sonnet 5.5: $2 / $10.
  - Claude Haiku 5.5: $0.10 / $0.50.
  - All have 1M context, image input and tool use.
  - Opus 5.5 **doesn't support forcing a specific tool** (`tool_choice` any/tool): use `auto`
    + `strict: true` schemas.
- **Browser use:** the official TypeScript SDK runs in the browser with
  `dangerouslyAllowBrowser: true`. That's fine for *your own key on your own device*, never for
  a shipped key.
- **Prompt caching:** the language reference + node catalog (~10–20k tokens) sits in a stable
  cached prefix. The minimum cacheable prefix on the current models is 512 tokens, and cache
  reads cost a fraction of normal input.
- **MCP:** Claude Desktop and Claude Code can drive local MCP servers, so the desktop app can
  expose its commands over MCP and you can use your Claude subscription instead of API
  billing. `rmcp` is the Rust SDK.
- **WebMCP** (`document.modelContext`, W3C community-group incubation, Chrome early preview):
  websites registering tools for browser agents. Worth adopting once it ships unflagged, since
  our command registry maps 1:1.

---

## 8. Lessons → design decisions

1. **The definition is the product.** Store a small text program that *is* the dependency graph
   (Dynamo, CADIR, Procedura). Geometry is always regenerated from it.
2. **Agent writes structure, human tunes values.** Sliders and handles never call the AI
   (CADAM). The chat box also acts as a command line for simple things ("height 200").
3. **Constrained functional language** for the agent (P3D-Bench): no loops, no mutation, no I/O.
   That also makes AI-written definitions *safe to run* on your device.
4. **Ask or flag, never silently invent** critical dimensions (ProCAD). Unknown values become
   parameters marked *assumed*.
5. **Check after every edit, look before you answer:** evaluate → diagnostics → render views →
   one targeted fix at a time (Procedura).
6. **Everything is a command** (ArtCraft): one registry for UI, agent, CLI and MCP; every agent
   action is an undoable, reviewable diff.
7. **Local-first:** kernel on the device; AI optional; no account (unlike Zoo's cloud engine).
8. **Compose, don't write, a kernel:** `curvo` + `manifold-rust` + `i_overlay` (+ `fidget`
   later). Mesh-first solids, exact curves.
9. **Phone-first web UI, Rust engine, Tauri desktop.**
10. **Print-native:** toolpaths are geometry. Port EasyGCode's engine with golden tests; use 3MF
    and existing slicers for everything else.

---

## 9. Lessons from browser games

*(Added 14 Oct 2026, from the owner's question: "a lot of people are creating AAA games that
work in the browser. How is that possible, and can we learn from it?")*

**How it works:**
- **WebAssembly (WASM):** game engines written in C/C++/Rust are compiled to WASM, which runs
  at near-native speed in any modern browser. Tools like Emscripten map threads, memory and
  file systems onto browser APIs.
- **WebGPU:** the successor to WebGL, built on the same modern GPU APIs as native games
  (Direct3D 12, Metal, Vulkan). Since late 2025 it's on by default in Chrome, Edge, Firefox and
  Safari, including **Safari on iOS 26**. Compute shaders let physics and other heavy maths
  run on the GPU; one reported case went from 8 to 60 fps by moving an algorithm to a compute
  shader.
- **Demos:**
  - Unreal Engine 5's *Lyra* demo reportedly runs fully in WASM + WebGPU.
  - The viral 2026 "browser ports" (Halo, Skate 3 and others) were made by decompiling the
    games, translating the code with AI, and **checking the translation against the original**.
    Many were taken down for using copyrighted assets.
- **Forecasts:** full-scale commercial AAA games in the browser are forecast for ~2027. The
  blockers are business and asset size as much as technology.

**What we take from it** (applied in [architecture.md §2.1](architecture.md#21-lessons-from-browser-games-why-browser-first-is-the-right-call)):
1. **Browser-first is viable even for heavy 3D.** EasyCAD is a web app; installers are optional.
2. **Compiled core + GPU drawing.** Rust → WASM for the engine; three.js on the GPU.
   - WebGL2 now (universal).
   - **WebGPU** via three.js's `WebGPURenderer` when we need compute or huge toolpaths. It
     falls back to WebGL2 automatically on devices without WebGPU.
3. **Never block the main thread.** Engine in a Web Worker; newest request wins.
4. **Threads need "cross-origin isolation" headers** (COOP/COEP) for shared memory.
   - GitHub Pages can't set headers. The `coi-serviceworker` workaround adds them from a
     service worker.
   - Hosts like Cloudflare Pages or Netlify set them natively.
   - Single-threaded is plenty for Phase 0–2.
5. **Lazy-load the heavy parts and keep big data on the device** (OPFS / IndexedDB), like
   games stream assets and cache them.
6. **Port with verification.** The same "translate, then compare against the original" loop
   the browser ports use is how we bring EasyGCode's math into Rust (golden tests).

---

## Sources

**ArtCraft / Rust apps**
- ArtCraft suite overview (apps, command registry, MCP/CLI): https://www.besthub.dev/articles/rewriting-adobe-suite-in-rust-12-open-source-apps-56k-stars-built-for-ai-agents-051d286733fe
- Phoronix on PhotoCraft/FilmCraft: https://www.phoronix.com/news/Photocraft-Filmcraft-AI
- PhotoCraft repo (AGENTS.md, docs/architecture.md): https://github.com/storytold/photocraft
- CADCraft repo (AGENTS.md, docs/gaps.md, docs/geometry-parity.md): https://github.com/storytold/cadcraft
- Halfspace: https://www.mattkeeter.com/projects/halfspace/ · Fidget: https://github.com/mkeeter/fidget
- Graphite: https://github.com/GraphiteEditor/Graphite · desktop RC review: https://itsfoss.com/graphite-graphics-editor.md · overview: https://digitalproduction.com/2025/07/14/graphite-open-source-vector-design-tool-with-a-twist/

**AI CAD products**
- Zoo Design Studio v1: https://zoo.dev/blog/zoo-design-studio-v1 · Text-to-CAD: https://docs.zoo.dev/text-to-cad · modeling-app: https://github.com/KittyCAD/modeling-app
- CADAM: https://github.com/Adam-CAD/CADAM · Show HN: https://hn.svelte.dev/item/45140921 · write-up: https://themenonlab.blog/blog/cadam-text-to-cad-browser-claude-webassembly
- Formas.AI Cartesian: https://aecmag.com/cad/formas-ai-unveils-native-agentic-3d-modeller
- AI CAD software in 2026 overview: https://blog.texocad.ai/posts/ai-cad-software-2026
- Autodesk AU 2026: https://architosh.com/2026/10/au2026-autodesk-shifts-ai-from-chatbots-to-agentic-ecosystems/
- MakerWorld Parametric Model Maker: https://all3dp.com/4/bambu-labs-parametric-model-maker-brings-openscad-to-makerworld/

**Rhino / Grasshopper**
- RhinoMCP (fork): https://github.com/SerjoschDuering/rhino-mcp
- Grasshopper Parametric Design MCP: https://playbooks.com/mcp/alfredatnycu-grasshopper-parametric-design
- Cordyceps: https://lobehub.com/mcp/brookstalley-cordyceps
- Grasshopper 2 beta in Rhino 9: https://discourse.mcneel.com/t/rhino-beta-feature-grasshopper-2-beta/221336
- Dynamo DesignScript: https://primer2.dynamobim.org/8_coding_in_dynamo/8-1_code-blocks-and-design-script/2-design-script-syntax
- rhino3dm (npm, MIT): https://www.npmjs.com/package/rhino3dm
- Plasticity: https://en.wikipedia.org/wiki/Plasticity_(software)

**Research papers**
- P3D-Bench: https://arxiv.org/abs/2606.11152
- Text2CAD-Bench: https://arxiv.org/abs/2605.18430
- ProCAD, "Clarify Before You Draw": https://arxiv.org/abs/2602.03045
- Procedura: https://arxiv.org/abs/2608.26238
- CADIR: https://arxiv.org/abs/2608.00891
- BenchCAD (not read in detail): https://arxiv.org/abs/2605.10865

**Geometry libraries**
- curvo: https://github.com/mattatz/curvo
- manifold-rust: https://github.com/larsbrubaker/manifold-rust · original Manifold: https://github.com/elalish/manifold
- truck: https://github.com/ricosjp/truck · Fornjot: https://github.com/hannobraun/fornjot
- i_overlay: https://crates.io/crates/i_overlay · kurbo: https://crates.io/crates/kurbo
- boolmesh: https://github.com/komietty/boolmesh · brepkit (AGPL): https://github.com/andymai/brepkit · opencascade-rs: https://github.com/bschwind/opencascade-rs

**Platforms**
- WebGPU in all major browsers incl. iOS 26: https://web.dev/blog/webgpu-supported-major-browsers
- egui 0.36 release notes (mobile keyboard/IME on web): https://newreleases.io/project/github/emilk/egui/release/0.36.0
- Tauri 2: https://v2.tauri.app

**3D printing**
- FullControl GCode Designer: https://repository.lboro.ac.uk/articles/journal_contribution/FullControl_GCode_Designer_Open-source_software_for_unconstrained_design_in_additive_manufacturing/15097083
- Kiri:Moto: https://github.com/GridSpace/grid-apps · https://docs.grid.space/projects/kiri-moto
- OrcaSlicer CLI: https://www.orcaslicer.com/wiki/cli/cli_mode
- Moonraker configuration (`cors_domains`, `trusted_clients`): https://moonraker.readthedocs.io/en/latest/configuration/
- Chrome Local Network Access: https://developer.chrome.com/blog/local-network-access
- Bambu third-party integration / Developer Mode: https://wiki.bambulab.com/en/software/third-party-integration

**Browser games (§9)**
- WebGPU in all major browsers: https://videocardz.com/newz/webgpu-is-now-supported-by-all-major-browsers
- "AI browser games" ports explained (WASM, Emscripten, decompile + verify): https://dev.to/jacksam/playing-halo-in-a-browser-tab-deconstructing-the-viral-ai-browser-games-phenomenon-70l
- AAA in the browser / Lyra demo claims (agency article, June 2025): https://riven.ch/en/news/jeux-aaa-dans-le-navigateur-web-webgpu
- Cross-origin isolation without server headers (coi-serviceworker): https://dev.to/stefnotch/enabling-coop-coep-without-touching-the-server-2d3n · GitHub Pages header request: https://github.com/orgs/community/discussions/13309
- three.js WebGPURenderer (automatic WebGL2 fallback): https://threejs.org/manual/en/webgpurenderer

**AI platform**
- Anthropic TypeScript SDK (browser use via `dangerouslyAllowBrowser`): https://github.com/anthropics/anthropic-sdk-typescript
- Models & pricing: https://platform.claude.com/docs/en/about-claude/pricing
- Prompt caching: https://platform.claude.com/docs/en/build-with-claude/prompt-caching
- rmcp (Rust MCP SDK): https://crates.io/crates/rmcp
- WebMCP explainer: https://zuplo.com/blog/what-is-webmcp
