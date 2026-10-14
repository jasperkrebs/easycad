# EasyCAD

**A lightweight, agent-first parametric modeller for makers and 3D printing.**
Describe what you want → an AI agent builds a *parametric definition* (a Grasshopper-style
system) → you tune it with sliders and handles → export it, or print it directly.

> Status: **research & architecture phase.** No app yet. Start with the docs below; the next
> step is Phase 0 of the [roadmap](docs/roadmap.md).

---

## The idea

CAD tools today were designed decades ago and have been extended ever since. EasyCAD starts
from what 2026 makes possible:

- **The agent builds the system, you own the values.** The agent writes a small, readable
  definition: parameters + named steps, like a Grasshopper definition as text. Every
  meaningful dimension and tolerance becomes a slider. Tuning never calls the AI: it's
  instant, free and offline.
- **Rhino/Grasshopper thinking, not feature trees.** Curves and NURBS first. Dataflow with
  automatic list handling (no loops). Some SolidWorks-style features (fits, shells,
  constraints) come later.
- **Made for 3D printing.** Watertight solids → 3MF for any slicer, and **curves can be
  toolpaths** printed directly, bringing [EasyGCode](https://github.com/jasperkrebstc/easygcode)'s
  engine inside.
- **Light and local.** A Rust engine that runs on your phone (as a web app, no install) and as
  a small desktop app. No account, no cloud kernel, works offline. AI is optional and uses
  your own key or Claude via MCP.

```
easycad 0.1
param height = 160 mm in 60..300 "Height"
param twist  = 90 deg in 0..360  "Twist"

base     = star(7, outer: 45 mm, inner: 37 mm) |> smooth(0.5)
t        = range(0, 1, count: 32)
sections = base |> rotate(twist * t) |> move(z: height * t)
vase     = loft(sections) |> cap()

show vase
```

## Docs

| | |
|---|---|
| [docs/research.md](docs/research.md) | What exists, what doesn't, what we build on, with sources and a measured kernel spike |
| [docs/architecture.md](docs/architecture.md) | The proposed architecture: engine, language, agent, UI, printing, distribution |
| [docs/definition-language.md](docs/definition-language.md) | Draft of the `.easycad` definition language, with examples |
| [docs/roadmap.md](docs/roadmap.md) | Step-by-step phases and the **decisions needed now** |
| [docs/mockup/index.html](docs/mockup/index.html) | Clickable UI mockup (open in a browser; works on a phone) |
| [spikes/kernel-probe](spikes/kernel-probe/) | Measured: pure-Rust NURBS + solids in WebAssembly (0.44 MB gz, ~50 ms) |
| [CLAUDE.md](CLAUDE.md) | Guidance for AI agents (and humans) working in this repo |

## Relationship to EasyGCode

EasyGCode keeps working as it is. EasyCAD generalises it: each EasyGCode project (vessel,
lampshade, thread, …) becomes an `.easycad` template, and its proven toolpath math is ported
with byte-for-byte golden tests. After that, the agent can create new "projects" without new
code.

## License

To be decided (recommended: MIT OR Apache-2.0; see the roadmap's decisions table).
