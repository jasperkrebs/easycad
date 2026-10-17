# CLAUDE.md: working on EasyCAD

Guidance for Claude Code (and humans) working in this repo. Read this first.

## What this is

EasyCAD is "EasyGCode without fixed tabs": a lightweight, agent-first parametric modeller for
**custom 3D-printing toolpaths**.
- An agent (or the user) writes a small dataflow project (`*.easycad`, a Grasshopper definition
  as text).
- The user tunes it in the project's settings window.
- The Rust engine turns shapes into curves, curves into toolpaths, and toolpaths into G-code
  (Bambu P1P filament, Klipper pellet).

**Current phase:** decisions answered (14–15 Oct 2026, see `docs/roadmap.md`). **Phase 0, 1a
(recipes as files), 1b (real G-code, golden-tested; EasyGCode's spiral disc as a recipe; STL),
1c (the Grasshopper toolbox, `spiralize`, the Graph tab) and 1d (every EasyGCode project from
general steps; thread and spoon golden, `docs/easygcode-coverage.md`) are done, and so is the
Chat tab (the agent with the user's key, behind a password proxy). Next: feature requests from
the agent, built by Claude Code (`docs/feature-requests.md`).** Hosting: public repo + GitHub Pages. Priorities:
- custom toolpaths and G-code;
- project files that reopen with their settings window;
- the agent with the user's own API key.

Slicers, 3MF and solids are "later".

## Read in this order

1. `README.md`: the idea
2. `docs/roadmap.md`: current phase, decisions, what's next
3. `docs/toolpaths-and-skills.md`: the core workflow (shapes → curves → toolpaths → G-code,
   skills, project files)
4. `docs/architecture.md`: how it's built and why
5. `docs/definition-language.md`: the project file / recipe language (§2: what works today)
   - `docs/easygcode-coverage.md`: which steps build each EasyGCode project, and what's missing
6. `docs/devlog.md`: what landed recently, what's open
7. `docs/research.md`: background and sources, when you need the "why"

## Build and run

```sh
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
scripts/build-web.sh               # wasm32 build + wasm-bindgen → web/pkg/ (fails over the size budget)
scripts/serve.sh                   # serves web/ at http://localhost:8080 (WASM can't load from file://)
node scripts/smoke-test.cjs [dir]  # headless Chromium, phone + desktop; screenshots into [dir]
node scripts/agent-test.cjs [dir]  # the Chat tab against a scripted stand-in for the AI
node scripts/graph-test.cjs [dir]  # the Graph tab: every kind of edit, mouse and touch
node --test agent-proxy/worker.test.mjs   # the AI proxy (Cloudflare Worker)
python3 scripts/hand-checked-fixtures.py --check   # hand-checked G-code fixtures up to date
python3 scripts/third-party-notices.py   # after adding/updating a dependency (CI runs --check)
```

CI (`.github/workflows/ci.yml`) runs all of the above. Look at the smoke-test screenshots for
any UI change.

- `web/pkg/` is generated; never edit it or commit it.
- `wasm-bindgen-cli` must match the `wasm-bindgen` version in `Cargo.lock` exactly.

## Non-negotiables

- **The definition is the single source of truth.** Geometry is always regenerated from the
  `.easycad` text plus parameter values. Graph edits, slider moves and agent edits all become
  edits to that text (graph edits through `easycad_core::graph` commands, which touch only
  their own piece of the text and are parsed again before use).
- **Everything is a command.** UI, in-app agent, CLI and MCP go through one command registry.
  No feature exists only in the UI.
- **Layering.** Engine crates never depend on UI, browser, Tauri, file system or network code
  (see the layer table in `docs/architecture.md` §4). The UI talks only to the command API.
- **Never crash.** No `unwrap`/`expect`/`panic!`/`todo!` in non-test Rust, no `unsafe`. Bad
  input of any kind becomes an error message. Every crash fix comes with a regression test.
- **Never break wasm.** Everything up to the engine must build for `wasm32-unknown-unknown`,
  and stay inside the size budget.
- **Tuning never needs the AI.** Parameter changes, exports and printing are local,
  deterministic and instant. The AI is optional.
- **Phone-first UI in the EasyGCode style** (see `docs/architecture.md` §9.4 for the tokens).
  Plain HTML/CSS/ES modules, no framework, no bundler. Third-party UI code is vendored and
  pinned in `web/vendor/`.
- **Licenses.** EasyCAD is all rights reserved: the repo is public, but not open source.
  Dependencies must be permissive (MIT/Apache/BSD/0BSD; MPL-2.0 OK). No GPL/AGPL in the shipped
  app. Add every new runtime dependency (Rust crate or vendored JS) to `THIRD-PARTY-NOTICES.md`
  in the same change.
- **G-code safety.** G-code drives real machines. Never emit printable G-code from a node
  unless it's golden-tested against EasyGCode (or a hand-checked fixture), and keep start/end
  blocks identical to EasyGCode's proven ones. Fixtures come from EasyGCode itself
  (`scripts/easygcode-fixtures.cjs` → `fixtures/easygcode/`); EasyGCode is never changed.
  Hand-checked fixtures (`fixtures/hand-checked/`) are worked out from the geometry in exact
  arithmetic by `scripts/hand-checked-fixtures.py`, independently of the Rust code.
- **Speeds as volumetric flow.** Templates and the agent set `flow:` (mm³/s), not `speed:`.
- **General steps, not per-project code.** A new project is a recipe. Add an engine step only
  when no combination of existing steps can do the job, and make it reusable (see
  `docs/creating-shapes.md`).

## Conventions (once code exists)

- **Rust:** edition 2024, `cargo fmt`, `cargo clippy -- -D warnings`, tests with every change.
  Golden tests for geometry (volume/bounds/hash) and G-code (exact text).
- **JavaScript:** ES modules, 2-space indent, small pure helpers, JSDoc types. Comments explain
  geometry and intent, as in EasyGCode.
- **Docs move with the code.** When behaviour changes, update the relevant doc in the same
  change. Add a short entry to the dev log (what landed, what's still open).
- `spikes/` are throwaway experiments with written results. Never import them from the app.

## Collaboration workflow

As in EasyGCode:
- Sync `main` before starting and before opening a PR.
- Keep branches short-lived and PRs small.
- Resolve conflicts by keeping the non-overlapping changes from both sides.
