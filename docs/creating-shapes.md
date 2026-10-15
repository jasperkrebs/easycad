# How shapes get made

*Written 15 Oct 2026, from the owner's question: "Now you created this star shape with a twist,
but my idea is that I explain it to an agent and the agent builds it every time. Maybe there
are other ways, because with something complex, using AI to build it every time… Maybe we need
basic CAD tools, but basic CAD tools are hard on a phone."*

## The key idea: the agent builds the machine, not the product

The twisted vase in Phase 0 is a **recipe**: a few steps (star → smooth → twist and scale →
wall → spiral toolpath) plus the settings that drive them. From Phase 1 on, a recipe is a
small file (`.easycad`), not code inside the app.

- **Building a recipe is the creative step.** It happens once, by you, by the agent, or both
  together.
- **Running a recipe is free.** Every slider move, every new toolpath and every G-code export
  just re-runs the recipe on your device, in milliseconds, offline, without AI.

So the agent does **not** build the shape every time. It writes (or edits) the recipe once,
in about half a minute for a few cents. After that the recipe is yours: saved as a project,
reopened with its own settings window, reused as a template. You only need the AI again to
change the *structure*: add a lid, add holes, switch from waves to spikes, or "make the lip
thickness a setting too".

That's exactly how Grasshopper works. The definition takes effort once; then sliders are
instant. The difference is that you can describe the definition instead of wiring it.

## When does the app itself have to change?

Rarely. A new *project* is a new recipe, never new app code: the spiral disc, which needed its
own generator in EasyGCode, is ten recipe lines in EasyCAD, with the same G-code (see
[toolpaths-and-skills.md](toolpaths-and-skills.md#proof-easygcodes-spiral-disc-built-inside-the-app-phase-1b-15-oct-2026)).

The engine only changes when a recipe needs a *kind* of step that doesn't exist yet: a boolean,
an offset, a new way of turning a surface into curves. Those steps are general on purpose, so
each one makes many new projects possible. Think of the steps as a vocabulary: the more words
there are, the more you (and the agent) can say without calling a programmer. Asking for "a
spiral that gets denser near the rim" needs no new word; "cut a hole through the wall" needs
`difference`, once.

## Five ways to make a shape (mix freely)

| Way | Who does the work | Good on the phone? | When |
|---|---|---|---|
| **1. Start from a template** | Nobody: open it, tune the settings | ✅ | Most days: your own past projects plus EasyGCode's projects as templates |
| **2. Describe it** | The agent writes or edits the recipe | ✅ (typing or dictating) | New ideas, variations ("like my lampshade, but 12 waves and a wider throat"), anything you can say but not easily build |
| **3. Build with steps** | You, from a menu of steps: the **Graph** tab (since 15 Oct), Grasshopper-style components and wires | ✅ (tap a component, edit its inputs in a list below) | Structural edits without AI: add a component, wire it, swap "star" for "polygon", add a slider |
| **4. Draw curves** | You, with your finger or mouse in 2D | ✅ (2D only) | Profiles, outlines and paths, like EasyGCode's drag-editable radius profile |
| **5. Bring something in** | The agent or an importer | ✅ | A photo of a sketch (the agent traces it), SVG/DXF curves from Rhino or Illustrator |

They all produce **the same recipe**. That's why they mix:

1. The agent makes a first version.
2. You drag a profile point.
3. You add a "wave" step from the menu.
4. You ask the agent to "add a 3 mm lip at the top".
5. You save it as your own template.

Nothing is locked to the way it was made.

## Why there's no "classic CAD" on the phone

Classic CAD means clicking edges, faces and 3D points precisely, and that's painful with a
finger. EasyCAD avoids it:

- **3D is for looking:** orbit, zoom, tap to see what a part is.
- **2D is for drawing:** curves live on flat canvases (top view, side profile) where fingers
  are precise. EasyGCode already proves this with its point editors.
- **Structure is a list:** the recipe shows as a vertical list of steps, each with its
  settings, like iOS Shortcuts or a modifier stack. Adding a step is "+ → pick from a menu".
  On the computer, the same recipe can also show as a Grasshopper-style graph.
- **Precision is numbers:** every dimension is a setting you type or slide, never something you
  "eyeball" in 3D.

## How this shows up in the app

```
┌─ Project: Twisted vase ─────────────────────── [Templates] [Save] ─┐
│  3D view                                                            │
├─────────────────────────────────────────────────────────────────────┤
│ [Settings]  [Steps]  [Draw]  [Recipe]  [Print]                      │
│  Settings: generated from the recipe's settings (sliders, numbers)  │
│  Steps:    the recipe as a list of steps (+ add, reorder, remove)   │
│  Draw:     2D curve editors for profile / outline settings          │
│  Recipe:   the text itself (what the agent reads and writes)        │
│  Print:    toolpath stats, checks, G-code export                    │
├─────────────────────────────────────────────────────────────────────┤
│ 💬 "add 9 waves to the wall"  or  "height 200"                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Order of building

1. **Phase 1a** ✓ (landed 15 Oct): recipes become files. Templates, a settings window
   generated from the recipe, open/save projects, the Recipe tab. *(This makes ways 1 and 2
   possible: the agent needs a recipe format to write into.)* Way 2 already works by hand: the
   Recipe tab's **Copy for AI** copies the recipe with a summary of the language and every
   step. Paste that into any Claude chat, describe what you want, and paste the answer back.
2. **Phase 1b** ✓ (landed 15 Oct): real G-code for the P1P and the Klipper pellet printer,
   golden-tested against EasyGCode; fields and general toolpath steps; EasyGCode's spiral disc
   as a recipe; STL export.
3. **Phase 2** (the Chat tab landed 15 Oct): the agent (way 2), writing and editing recipes;
   skills next.
4. **The Grasshopper toolbox and the Graph tab** ✓ (15 Oct, way 3): about 70 general steps
   named after Grasshopper's components (points, planes, transforms by fields, tweens,
   `spiralize`, …). The recipe can be built and edited as components and wires, with no AI at
   all, and the AI's recipes open in the same graph to be taken further by hand.
5. **Next:** 2D curve editors (way 4); importing curves and photos (way 5).

## Why a vocabulary, not one tool per project (15 Oct)

The owner's test request, "a cylinder whose top is cut by a tilted plane, with the layers in
between tweened and printed as one spiral", couldn't be built at first. The answer wasn't a
"cut cylinder" step. It was the missing *words*:
- planes and 3D rotation;
- moving and turning point by point (fields: `move(z: 0.1 * x)`);
- `tween`;
- `spiralize`, vase mode through any layers.

With those, the request is five lines (the "Cut cylinder" template), and the same words build
many other things: tilted rims, twisted or flared walls, non-planar layers.

When a request still can't be built, the AI says which word is missing rather than faking it.
That word gets added once, as a general step.
