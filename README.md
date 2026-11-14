# EasyCAD

**EasyGCode without fixed tabs:** a lightweight, agent-first parametric modeller for custom
3D-printing toolpaths. Describe a project (or build it yourself) → an AI agent builds a
*parametric recipe* with its own settings window → you tune it → shapes become curves, curves
become toolpaths, toolpaths become G-code for your printer.

> **Status (14 Nov 2026): live and in daily use** at https://jasperkrebs.github.io/easycad/, for
> large pellet prints on the Ginger G1 and filament prints on the Bambu P1P.
> - **Projects are recipes:** small `.easycad` text files that reopen with their own settings
>   window; one project can print several parts. The Rust engine runs in the browser.
> - **Templates:** every EasyGCode project (the lampshade, bend stool, thread connector and
>   spiral disc with EasyGCode's exact G-code; the vessel), the salt & pepper shaker, and the
>   **Lamp**: shades that screw into each other ([docs/lamp-builder.md](docs/lamp-builder.md)).
> - **G-code** for the G1 and the P1P, speeds as volumetric flow, golden-tested against
>   EasyGCode ([docs/printers.md](docs/printers.md)); **STL** and **STEP** exports.
> - **The views:** shape and toolpath, the Plan (tap a part to reach its settings), the prints
>   overview, playback, beads as they'd lie, the nozzle along the wall.
> - **Chat tab:** Claude writes and edits recipes with your own key; when the steps can't do
>   something it asks for the missing step ([docs/feature-requests.md](docs/feature-requests.md)).
>
> What's open and what's next: the [roadmap](docs/roadmap.md).

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
  **Bambu P1P** or the **Ginger Additive G1** pellet printer.
- **The agent builds the system; you own the values.** The agent writes the recipe using
  *skills* (your recurring ways of doing things). Every number becomes a setting. Tuning never
  calls the AI: it's instant, free and works offline.
- **Light and local.** A Rust engine compiled to WebAssembly runs on your phone and computer,
  in the browser, installable like an app. No account, no cloud kernel. AI uses your own Claude
  API key ([how to get one](docs/api-key.md)).

A small project, exactly as the app saves it (every `param` is a slider, every `group` a fold-out):

```
easycad 0.1
project "Twisted star vase"
printer "Bambu P1P 0.4"
note "A rounded star that shrinks and twists as it rises, printed as one continuous spiral (vase mode)."

group "Shape"
param height    = 160 mm  in 40..300          "Height"
param radius    = 45 mm   in 10..150          "Tip radius"
param points    = 7       in 3..16 step 1     "Star points"
param depth     = 18 %    in 0..60            "Star depth"
param twist     = 90 deg  in -360..360        "Twist"
param top_scale = 0.8     in 0.3..2           "Top scale"

group "Print"
param layer     = 0.3 mm  in 0.08..3          "Layer height"
param line      = 0.6 mm  in 0.2..10          "Line width"
param flow      = 8       in 1..25            "Flow (mm³/s)"

# One rounded star, copied every few degrees of twist: each copy scaled, turned and lifted.
outline  = star(points, outer: radius, inner: radius * (1 - depth)) |> smooth
t        = range(0, 1, count: 16 + abs(twist) / 3)
sections = outline
           |> scale(lerp(1, top_scale, t))
           |> rotate(twist * t)
           |> move(z: height * t)
wall     = loft(sections)
vase     = cap(wall)
path     = vase_spiral(wall, layer: layer, width: line, flow: flow)

show vase
print path
export vase as "twisted-vase.stl"
```

## Docs

| | |
|---|---|
| [docs/roadmap.md](docs/roadmap.md) | Phases, decisions, what's next |
| [docs/toolpaths-and-skills.md](docs/toolpaths-and-skills.md) | **The core workflow:** shapes → curves → toolpaths → G-code, skills, project files |
| [docs/creating-shapes.md](docs/creating-shapes.md) | How shapes get made: the agent builds the recipe once; five ways to make one |
| [docs/architecture.md](docs/architecture.md) | How it's built and why (incl. lessons from browser games) |
| [docs/definition-language.md](docs/definition-language.md) | The `.easycad` project file / recipe language (v0.1) |
| [docs/research.md](docs/research.md) | What exists, what doesn't, sources, measured kernel spike |
| [docs/api-key.md](docs/api-key.md) | Connecting the AI: your Claude API key, the password-protected proxy, costs |
| [docs/easygcode-coverage.md](docs/easygcode-coverage.md) | Every EasyGCode project, feature by feature, and the steps that build it |
| [docs/printers.md](docs/printers.md) | The printers: the Ginger G1 from its wiki, its limits, materials and checks |
| [docs/lamp-builder.md](docs/lamp-builder.md) | Components and parts: lamps of shades that screw into each other |
| [docs/feature-requests.md](docs/feature-requests.md) | When the AI can't build something: it asks, Claude Code builds the step, the chat carries on |
| [docs/grasshopper.md](docs/grasshopper.md) | Research: toolpaths and recipes in Grasshopper, Grasshopper files in EasyCAD, building without the AI |
| [docs/devlog.md](docs/devlog.md) | What landed, what's open |
| [docs/working-together.md](docs/working-together.md) | Who works on what: the owner, the general-manager and recipe-developer agents, their branches |
| [CLAUDE.md](CLAUDE.md) | Guidance for AI agents (and humans) working in this repo |

## Develop

Requirements: Rust (stable, with the `wasm32-unknown-unknown` target) and `wasm-bindgen-cli`
(same version as the `wasm-bindgen` crate in `Cargo.lock`).

```sh
scripts/build-web.sh     # Rust → WebAssembly → web/pkg/
scripts/serve.sh         # http://localhost:8080
cargo test --workspace   # language and engine tests (golden G-code, damaged recipes)
```

Two more for the engine, run by hand: a long round of damaged recipes
(`FUZZ_CASES=5000 cargo test --release -p easycad-core --test fuzz`), and a survey of every
template with each setting at both ends of its range, with timings
(`cargo test --release -p easycad-core --test sweep -- --ignored --nocapture`).

Templates live in `web/templates/*.easycad`; the engine's tests run every one of them, and the
test-only recipes in `fixtures/recipes/` (the example above, EasyGCode's spoon).
`fixtures/easygcode/` holds G-code written by EasyGCode itself, for the golden tests.

Two Claude Code agents work on the repo, each on its own branch: `general-manager` (the app and
its architecture) and `recipe-developer` (templates and projects). Both start every task by
merging the newest `main` ([docs/working-together.md](docs/working-together.md)).

## Relationship to EasyGCode

EasyGCode stays exactly as it is. EasyCAD reuses its proven math, ported to Rust with
golden tests that require identical G-code. Each EasyGCode tab becomes a starter project made of
general steps (the spiral disc is the first), so new projects no longer need new code.

## License

All rights reserved: the repository is public, but EasyCAD is not open source. Third-party
components are permissively licensed; see [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
