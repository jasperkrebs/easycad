# Dev log

Newest first. Each entry: what landed, numbers, what's still open. Keep it short; it's how the
next session (human or agent) picks up.

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
