# Dev log

Newest first. Each entry: what landed, numbers, what's still open. Keep it short; it's how the
next session (human or agent) picks up.

## 2026-10-15: Phase 1a, recipes become files

**Landed**
- **Hosting decided:** the repo is public, and CI deploys `web/` to GitHub Pages from the
  default branch. It needs one setting from the owner (Settings → Pages → Source: GitHub
  Actions); until then the deploy job leaves a notice and stays green.
- **`easycad-lang` (new crate):** lexer, parser and syntax tree for the recipe language v0.1.
  - Errors carry line and column and read like advice ("`in` is a reserved word; pick another
    name").
  - `set_values` writes setting values back into the text and touches nothing else: comments,
    alignment and other lines stay as they were.
  - Hostile input can't crash it: nesting and expression size are capped.
- **`easycad-core` runs recipes:**
  - the evaluator runs steps in dependency order (Kahn's algorithm, no recursion). It reports
    unknown names (with "did you mean"), loops and duplicates. An error belongs to its step; steps
    that need it are skipped and everything else still runs;
  - lists replicate Grasshopper-style (longest list), units convert to mm, degrees and ratios,
    and a budget caps how much geometry one run can make;
  - 24 nodes:
    - numbers and lists: `range`, `lerp`, `count`, `min`, `max`, `clamp`, `mod`, `abs`, `sqrt`,
      `round`, `floor`, `ceil`, `sin`, `cos`, `tan`;
    - curves: `circle`, `polygon`, `star`, `smooth`;
    - transforms: `scale`, `rotate`, `move`;
    - surfaces: `loft`, `cap`;
    - toolpaths: `vase_spiral`, `wave`;
    - plus a catalog generated from the code;
  - the hard-coded Phase 0 demo is gone. The twisted vase is now a recipe and prints the same:
    path length 135,442.45 mm vs 135,442.76 before. With no twist and the top at 2×, it matches
    to every printed digit;
  - the overhang check only searches the nearby part of the layer below (7.7 → 2.2 ms).
- **Web app:**
  - the settings window is generated from the recipe: sliders with number fields, toggles,
    dropdowns, text fields, groups, labels, ranges, steps and an "assumed" badge;
  - **Recipe tab:** the text itself, with Run and Revert:
    - problems listed by line (tap one to jump there);
    - every step and what it made ("list of 46 curves");
    - the step catalog;
    - **Copy for AI**: the recipe plus a summary of the language and the catalog, to paste into
      any Claude chat until the agent lives in the app;
  - templates: twisted star vase, ribbed cylinder (a wave on the toolpath), star bowl;
  - open/save `.easycad` and save/load settings (JSON). The project is autosaved on the device;
    "Reset settings" goes back to the values as opened;
  - the engine worker restarts itself if the WebAssembly ever traps;
  - text from a recipe only reaches the page as text, never as HTML, because recipes can come
    from anywhere.

**Numbers** (cloud CPU)
- Engine WASM: 573 KB raw / **173 KB gzipped** (Phase 0: 80 KB; budget 3 MB).
- Twisted vase, full quality (46 sections, 533 layers, 119,617 path points): **14 ms** native,
  **26 ms** as WebAssembly in Node. The preview while dragging takes 7 / 12 ms.
- 68 Rust tests (17 language, 51 engine), including a random-recipe crash test, the Phase 0
  parity test and a check that every node is in `docs/definition-language.md`. The smoke test is
  green at phone and desktop sizes: it edits a setting, edits and breaks the recipe, switches
  templates and reloads.

**Open**
- **Owner:** switch on GitHub Pages (one setting, see above).
- **1b:** G-code for the P1P and the Klipper pellet printer, golden-tested against EasyGCode; a
  solid bottom (walls only for now); the vessel template.
- **1c:** formatter, presets, a project library on the device, `revolve` / `offset_rings` /
  `contours`.
- No per-step caching yet: every run recomputes everything, which is fine at these sizes.
- Language features still on paper: domains, comparisons and `if`, records, presets.

## 2026-10-14: Phase 0 walking skeleton

**Landed**
- Rust workspace:
  - edition 2024, no-panic lints (`unwrap`/`expect`/`panic` denied outside tests), `unsafe`
    forbidden;
  - `easycad-core`, the engine: twisted star vase demo → preview mesh, continuous vase-mode
    toolpath (start ramp, whole-layer snapping, flat cap, as in EasyGCode), print stats;
  - `easycad-wasm`, a thin wasm-bindgen facade.
- Web app (`web/`) in the EasyGCode look:
  - engine in a Web Worker with latest-wins scheduling;
  - three.js 0.186.1 vendored (WebGL 2), toolpath as fat lines coloured by height;
  - settings with sliders + number fields, preview while dragging, full quality on release;
  - print estimate and checks;
  - save/load settings (JSON) and settings remembered on the device;
  - PWA manifest, network-first service worker, generated icons.
- Scripts: `build-web.sh` (with size budget), `serve.sh`, `smoke-test.cjs` (headless
  Chromium, phone + desktop), `third-party-notices.py` (license check + notices).
- CI (`.github/workflows/ci.yml`): fmt, clippy, tests, wasm check, notices check, web build,
  smoke test. Pages deploy is ready but gated on the `DEPLOY_PAGES` repo variable.
- Docs: decisions recorded (roadmap). New `toolpaths-and-skills.md`, `api-key.md`, and
  research §9 (browser games).

**Numbers** (cloud CPU, headless Chromium)
- Engine WASM: 267 KB raw / **80 KB gzipped** (budget 3 MB).
- Full-quality vase (twist 180°: 120 rings × 224 samples, 533 layers, ~120k path points):
  **~15 ms** per evaluation in the worker.
- 15 Rust tests, all green. Smoke test green on phone and desktop sizes.

**Open**
- **Hosting:** the repo is private. Pages needs GitHub Pro (or a public repo, or Cloudflare
  Pages). Owner to decide; then set `DEPLOY_PAGES=true`.
- Phase 1 next: definition language v0, evaluator, settings window generated from the
  project, project files, first G-code (P1P + pellet) golden-tested against EasyGCode.
