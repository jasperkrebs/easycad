# EasyCAD

**EasyGCode without fixed tabs:** a lightweight, agent-first parametric modeller for custom
3D-printing toolpaths. Describe a project (or build it yourself) → an AI agent builds a
*parametric recipe* with its own settings window → you tune it → shapes become curves, curves
become toolpaths, toolpaths become G-code for your printer.

> **Status:** Phase 0 done (14 Oct 2026).
> - **Working:** the Rust engine runs in the browser as WebAssembly. A twisted-vase demo
>   shows the shape, its continuous print toolpath and a live print estimate.
> - **Next:** Phase 1, project files with their own settings window, and real G-code.
>
> See the [roadmap](docs/roadmap.md).

---

## The idea

- **Every project has its own settings window,** like an EasyGCode tab, except you and the agent
  create it inside the app. Save the project, reopen it, and the same window comes back with
  the same settings.
- **Shape first or curve first.** Shapes turn into curves with operations, not with a slicer:
  - spiral along any surface;
  - offset rings with zipper seams;
  - non-planar patterns;
  - unroll flat.

  Curves turn into toolpaths with EasyGCode's bead model, then into **custom G-code** for a
  **Bambu P1P** or a **Klipper pellet** printer.
- **The agent builds the system; you own the values.** The agent writes the recipe using
  *skills* (your recurring ways of doing things). Every number becomes a setting. Tuning never
  calls the AI: it's instant, free and works offline.
- **Light and local.** A Rust engine compiled to WebAssembly runs on your phone and computer,
  in the browser, installable like an app. No account, no cloud kernel. AI uses your own Claude
  API key ([how to get one](docs/api-key.md)).

```
easycad 0.1
project "Twisted vase"
printer "Bambu P1P"

param height = 160 mm in 60..300 "Height"
param twist  = 90 deg in 0..360  "Twist"

base     = star(7, outer: 45 mm, inner: 37 mm) |> smooth(0.5)
t        = range(0, 1, count: 32)
sections = base |> rotate(twist * t) |> move(z: height * t)
wall     = loft(sections)
path     = spiral_on(wall, pitch: 0.3 mm) |> toolpath(width: 0.6 mm, height: 0.3 mm)

print path
```

## Docs

| | |
|---|---|
| [docs/roadmap.md](docs/roadmap.md) | Phases, decisions, what's next |
| [docs/toolpaths-and-skills.md](docs/toolpaths-and-skills.md) | **The core workflow:** shapes → curves → toolpaths → G-code, skills, project files |
| [docs/architecture.md](docs/architecture.md) | How it's built and why (incl. lessons from browser games) |
| [docs/definition-language.md](docs/definition-language.md) | The `.easycad` project file / recipe language (draft) |
| [docs/research.md](docs/research.md) | What exists, what doesn't, sources, measured kernel spike |
| [docs/api-key.md](docs/api-key.md) | Getting and using your Claude API key |
| [docs/devlog.md](docs/devlog.md) | What landed, what's open |
| [docs/mockup/index.html](docs/mockup/index.html) | Clickable UI mockup |
| [CLAUDE.md](CLAUDE.md) | Guidance for AI agents (and humans) working in this repo |

## Develop

Requirements: Rust (stable, with the `wasm32-unknown-unknown` target) and `wasm-bindgen-cli`
(same version as the `wasm-bindgen` crate in `Cargo.lock`).

```sh
scripts/build-web.sh     # Rust → WebAssembly → web/pkg/
scripts/serve.sh         # http://localhost:8080
cargo test               # engine tests
```

## Relationship to EasyGCode

EasyGCode stays exactly as it is. EasyCAD reuses its proven math, ported to Rust with
golden tests that require identical G-code. Each EasyGCode tab becomes a starter project, so
new projects no longer need new code.

## License

Private, all rights reserved. Third-party components are permissively licensed; see
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
