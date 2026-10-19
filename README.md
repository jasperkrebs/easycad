# EasyCAD

**EasyGCode without fixed tabs:** a lightweight, agent-first parametric modeller for custom
3D-printing toolpaths. Describe a project (or build it yourself) → an AI agent builds a
*parametric recipe* with its own settings window → you tune it → shapes become curves, curves
become toolpaths, toolpaths become G-code for your printer.

> **Status:** Phase 1f done (19 Oct 2026). Try it: https://jasperkrebs.github.io/easycad/
> - **Projects are recipes:** small `.easycad` text files. Each one opens with its own
>   settings window, generated from the recipe (sliders, toggles, dropdowns, and tables for
>   list settings like radii at heights). One project can print several parts (a container
>   and its lid), each its own G-code file. The Rust engine runs in the browser as
>   WebAssembly, shows the shape and its toolpath, and gives a live print estimate.
> - **Graph tab:** the recipe as Grasshopper-style components and wires. Build and edit
>   without any AI (add components, wire inputs, sliders), or take what the AI made further by
>   hand. About 100 steps, named after Grasshopper's components where one exists: points,
>   planes, tweens, transforms by fields (per point), surfaces, vase mode through tilted
>   layers. Plus EasyGCode's techniques as general steps: ring fills, helices, zone speeds,
>   fan levels, dwells, spikes and overhang pacing.
> - **Chat tab:** Claude writes and edits recipes in the app, with your own key (behind your
>   own password, or stored on your device; see [docs/api-key.md](docs/api-key.md)). It can
>   switch printers and change printer settings too. When the steps can't do something, it
>   asks for the missing step. Claude Code builds it in GitHub Actions, and the chat carries on
>   when it's live ([docs/feature-requests.md](docs/feature-requests.md)).
> - **G-code** for the Bambu P1P and the **Ginger Additive G1** pellet printer (Klipper),
>   written the way EasyGCode writes it, speeds set as volumetric flow, golden-tested against
>   EasyGCode's own output. The app knows the G1 from Ginger's wiki: its bed, nozzles, flow
>   window, heater limits and material profiles ([docs/printers.md](docs/printers.md)).
>   **STL** for plain geometry.
> - **Templates:** EasyGCode's spiral disc, thread connector, spoon, bend stool (with foaming)
>   and lampshade (every shade, or your own) as recipes with the same G-code as EasyGCode; its
>   vessel (the bottom rounding into the wall, a non-planar star top) and container (as a salt &
>   pepper shaker: a lid with holes that threads onto it, two prints) from general steps; a
>   cylinder cut by a tilted plane (tilted layers in one spiral), a geometry-only turned vase,
>   and three vase-mode vessels. Every EasyGCode project can be built from the steps
>   ([docs/easygcode-coverage.md](docs/easygcode-coverage.md)).
> - **Machine control** as steps: temperatures and waits, speed and extrusion overrides, fan
>   levels, pauses (EasyGCode's foaming is built from them).
> - **Next:** the coat hanger as a template, skills for the agent, and 2D curve
>   editors.
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
  **Bambu P1P** or the **Ginger Additive G1** pellet printer.
- **The agent builds the system; you own the values.** The agent writes the recipe using
  *skills* (your recurring ways of doing things). Every number becomes a setting. Tuning never
  calls the AI: it's instant, free and works offline.
- **Light and local.** A Rust engine compiled to WebAssembly runs on your phone and computer,
  in the browser, installable like an app. No account, no cloud kernel. AI uses your own Claude
  API key ([how to get one](docs/api-key.md)).

A project, exactly as the app saves it (the first template; every `param` is a slider):

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
| [docs/feature-requests.md](docs/feature-requests.md) | When the AI can't build something: it asks, Claude Code builds the step, the chat carries on |
| [docs/devlog.md](docs/devlog.md) | What landed, what's open |
| [docs/mockup/index.html](docs/mockup/index.html) | Clickable UI mockup |
| [CLAUDE.md](CLAUDE.md) | Guidance for AI agents (and humans) working in this repo |

## Develop

Requirements: Rust (stable, with the `wasm32-unknown-unknown` target) and `wasm-bindgen-cli`
(same version as the `wasm-bindgen` crate in `Cargo.lock`).

```sh
scripts/build-web.sh     # Rust → WebAssembly → web/pkg/
scripts/serve.sh         # http://localhost:8080
cargo test --workspace   # language and engine tests
```

Templates live in `web/templates/*.easycad`; the engine's tests run every one of them.
`fixtures/easygcode/` holds G-code written by EasyGCode itself, for the golden tests.

## Relationship to EasyGCode

EasyGCode stays exactly as it is. EasyCAD reuses its proven math, ported to Rust with
golden tests that require identical G-code. Each EasyGCode tab becomes a starter project made of
general steps (the spiral disc is the first), so new projects no longer need new code.

## License

All rights reserved: the repository is public, but EasyCAD is not open source. Third-party
components are permissively licensed; see [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
