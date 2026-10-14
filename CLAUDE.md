# CLAUDE.md: working on EasyCAD

Guidance for Claude Code (and humans) working in this repo. Read this first.

## What this is

EasyCAD is a lightweight, agent-first parametric modeller for makers and 3D printing. An agent
writes a small dataflow definition (`*.easycad`, a Grasshopper definition as text); the human
tunes it with sliders; the Rust engine evaluates it to curves, solids and toolpaths.

**Current phase:** research and architecture are done (see `docs/`). Next is **Phase 0** in
`docs/roadmap.md`. Don't start building before the "Decisions needed now" in the roadmap are
answered by the owner.

## Read in this order

1. `README.md`: the idea
2. `docs/architecture.md`: how it's built and why
3. `docs/definition-language.md`: the document format the whole app revolves around
4. `docs/roadmap.md`: current phase, what's next, open decisions
5. `docs/research.md`: background and sources, when you need the "why"

## Non-negotiables

- **The definition is the single source of truth.** Geometry is always regenerated from the
  `.easycad` text plus parameter values. Graph edits, slider moves and agent edits all become
  edits to that text.
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
- **Licenses.** Dependencies must be permissive (MIT/Apache/BSD/0BSD; MPL-2.0 OK). No
  GPL/AGPL in the shipped app.

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
