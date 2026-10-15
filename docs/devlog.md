# Dev log

Newest first. Each entry: what landed, numbers, what's still open. Keep it short; it's how the
next session (human or agent) picks up.

## 2026-10-15: speeds as volumetric flow

**Landed**
- The toolpath steps take `flow:` (mm³/s). Each move's speed is flow ÷ its bead area and the
  G-code feed is flow · 60 ÷ area, rounded, as in EasyGCode's flow mode. Moves can also carry
  their own layer height (for non-planar layers, next), and the bead area follows it.
- Golden-tested against EasyGCode's flow mode: the spiral disc template (now `flow: 20` by
  default) and the vase spiral (`flow: 8`). Every command identical, six cases in all. The
  speed-mode cases still pass.
- All templates set flow instead of speed. The Print tab shows the flow and the nozzle speed it
  gives, and checks the flow against a new printer setting, "max flow" (P1P: 20 mm³/s).
- The agent's guide says to use flow, and each message tells it the printer's max flow.

## 2026-10-15: the agent in the app (Chat tab) and the AI proxy

**Landed**
- **Chat tab** (`web/js/chat.js`, `web/js/agent.js`): Claude builds and edits the recipe with five
  tools (`edit_recipe`, `write_recipe`, `set_settings`, `look`, `read_recipe`) that run in the
  browser on the engine. Each change runs at once and reports back as compact text; `look`
  returns one picture with four views (perspective, and front/side/top as orthographic views).
  Streaming answers, summarized thinking, an "Undo its changes" button per message, the cost of
  each answer and of the month, the chat kept on the device (IndexedDB), and "Your rules" that
  apply to every project.
- **Model:** Sonnet 5.5 at medium effort by default (Opus 5.5 and Haiku 5.5 in settings),
  adaptive thinking, refusal fallback on. The system prompt (language guide + step catalog,
  about 4k tokens) is byte-identical on every request and cached; the chat is append-only.
- **Two ways to connect:** a password for your own **AI proxy** (`agent-proxy/`, a Cloudflare
  Worker that holds the key; CI deploys it from four GitHub secrets and writes its address into
  the site's `config.json`), or a key stored on the device. Setup: `docs/api-key.md`.
- The official TypeScript SDK 0.129.0 is vendored as one minified ES module (189 KB, 50 KB
  gzipped), loaded only when the chat is used. "Copy for AI" now shares the agent's language
  guide (`web/js/language-guide.js`).
- **Tests:** `agent-proxy/worker.test.mjs` (password, origins, models, limits, streaming
  pass-through; the Claude API stubbed) and `scripts/agent-test.cjs` (the Chat tab against a
  scripted stand-in for the proxy and the API: write → look → edit → answer; checks caching
  settings, a fixed tool list, an append-only history, the picture, undo, reload, a wrong
  password). Both run in CI.

**Open**
- The owner sets up the secrets (or a key on the device); no real API call has been made from
  CI or here yet (no key in this environment). The first real chat is the real test.

## 2026-10-15: Phase 1b, real G-code, and EasyGCode's spiral disc as a recipe

**Why:** the owner wants to build custom things inside the app, both G-code projects like
EasyGCode's and plain geometry to export, without a new app build for every project. The test:
EasyGCode's spiral disc, which was a dedicated generator, rebuilt only from general steps.

**Landed**
- **G-code** (`core::gcode`) in EasyGCode's dialect for the P1P (filament, Marlin start/end) and
  the Klipper pellet printer: start and end blocks copied verbatim, E from the bead model
  (mm³ or mm of filament), numbers rounded exactly like JavaScript's `toFixed`, F only when it
  changes, the fan after the first layer. Before writing, it checks the bed, the build height
  and every extrusion amount; the end lift stops at the build height (Klipper refuses moves past
  it; EasyGCode doesn't check, so this is the one deliberate difference).
- **Golden tests against EasyGCode itself:** `scripts/easygcode-fixtures.cjs` runs EasyGCode's
  `gcode.js` headlessly and writes `fixtures/easygcode/`. **Every command identical** in all
  four cases:
  - three spiral disc cases (default pellet, filament with a multiplier and fan, no start/end
    blocks): 8,772, 9,639 and 5,133 commands;
  - `vase_spiral` on a cylinder against EasyGCode's lampshade generator with both ends the same
    size (its vase mode: ramped first turn, whole layers, flat closing turn): 7,908 commands.

  So every toolpath the templates print is covered. The browser smoke test also checks the
  G-code the app downloads for the spiral disc.
- **Fields:** numbers that vary over space. `distance(curve)`, `fade(…)` (EasyGCode's falloff),
  and arithmetic and the number steps on fields, point by point. `wave`'s amplitude can be one.
- **General toolpath steps:** `toolpath(curves, …)` prints any curves; `stack` repeats a layer
  (EasyGCode's lift-and-return layer change); `join` prints toolpaths one after another;
  `spiral` (RK4 like EasyGCode, spacing from a field) and `interlock` (the odd-bump wiggles,
  solved per angle like EasyGCode). 34 steps in all.
- **The spiral disc template** (`web/templates/spiral-disc.easycad`): ten lines, every setting
  of EasyGCode's tab, same G-code.
- **STL export:** `export shape as "x.stl"` (binary STL of surfaces and solids; the vase's is
  closed and faces outward, which a test checks). The vase and bowl templates offer one.
- **`core::files`** makes the files (G-code for `print`, the file of each `export`), so the
  app, a CLI and MCP share one path. The WebAssembly facade got `exportFile`.
- **Web app:**
  - Settings tab: an **Export** section (G-code for the chosen printer, plus the recipe's
    `export` files);
  - Print tab: the printer choice and **printer settings** (bed, where the recipe's origin sits,
    travel speed, multiplier, end lift, temperatures, fan, pellet purge and pressure advance),
    remembered per printer, with EasyGCode's defaults; checks for bed fit and height;
  - the 3D bed grid follows the printer's bed and origin;
  - the engine client answers requests by id (runs, exports, the catalog) and re-asks whatever
    a crashed worker never answered.

**Numbers** (cloud CPU, WebAssembly in Node)
- Engine WASM: 696 KB raw / **212 KB gzipped** (budget 3 MB).
- Spiral disc: **19 ms** per run (8,737 path points), G-code 28 ms (315 kB).
- Twisted vase: 25 ms per full-quality run; G-code 0.28 s (119,617 moves, 4.4 MB); STL 47 ms
  (2 MB).
- 95 Rust tests (17 language, 78 engine). Smoke test green on phone and desktop: it also saves
  an STL and checks the spiral disc's G-code against EasyGCode's.

**Open**
- **1c:** more of EasyGCode as recipes from general steps: the vessel (wall plus a solid bottom
  as a joined toolpath), the bend stool seat (`offset_rings`), the lampshade (`revolve`,
  `spiral_on`). Each golden-tested.
- Geometry for STL: `extrude`, `revolve`, `offset`, then booleans.
- No per-step caching yet; G-code for very large toolpaths takes a few tenths of a second.

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
- ~~**Owner:** switch on GitHub Pages~~ (done: live at https://jasperkrebs.github.io/easycad/).
- ~~**1b:** G-code for the P1P and the Klipper pellet printer~~ (done, see above); a solid
  bottom and the vessel template moved to 1c.
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
