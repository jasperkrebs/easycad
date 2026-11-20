# EasyCAD roadmap

**Where we are (14 Nov 2026):** EasyCAD is live and in daily use at
https://jasperkrebs.github.io/easycad/, for the owner's own prints on the Ginger G1 (pellets) and
the Bambu P1P. Work follows what the prints and the owner need, not a phase plan. Two agents build
it ([working-together.md](working-together.md)): **general-manager** (the app and its
architecture) and **recipe-developer** (templates and projects). How it got here: the dev log
archives ([October](devlog-2026-10.md), [November 1–14](devlog-2026-11.md)).

## What it does today

- **Recipes** (`.easycad`): a project is a small text file (settings and steps) that reopens
  with its settings window; templates for every EasyGCode project (spiral disc, thread
  connector, lampshade, bend stool, vessel) plus the salt & pepper shaker, the **Lamp** (shades
  that screw into each other, [lamp-builder.md](lamp-builder.md)) and the **Thread pendant** (the
  Thread Lampshade Sketcher's pendant, printable; it opens the sketcher's settings files,
  [thread-pendant.md](thread-pendant.md)).
- **G-code** for the G1 and the P1P with EasyGCode's start/end blocks; the EasyGCode projects
  golden-tested command for command; speeds as volumetric flow; bed, height and heater checks.
- **A robot program** (KUKA KRL) for the SPHR handover's KR50 pellet cell, the nozzle along the
  wall (`tilt`), with the handover's start and end ([printers.md](printers.md)).
- **The views:** shape, toolpath, both, the **Plan** (a cut through the axis: tap a part, its
  marks lead to their settings, add and remove shades), the **Prints** overview (each part on its
  own bed); colour modes, beads at their real size or simulated, the nozzle upright or along the
  wall, playback, the lamp taken apart, PNG export.
- **Exports:** G-code per part, STL, **STEP** (exact faces for shapes of revolution).
- **The AI** (Chat tab): Claude with the owner's key behind a password proxy, writing and editing
  recipes; when no combination of steps can do it, it files a feature request that a Claude Code
  run builds ([feature-requests.md](feature-requests.md)).

## Decisions that stand

| Question | Decision |
|---|---|
| Stack | Rust engine compiled to WebAssembly + plain HTML/JS app (no framework, no bundler); phone-first; a PWA. |
| Projects | Own text format, `.easycad`: the recipe is the single source of truth; every change (slider, agent, printer) is an edit to it. |
| Making things | General steps, not one generator per project: a new project is a recipe; a new step only when no combination of steps can do it. |
| Printers | Ginger G1 (Klipper, pellets) and Bambu P1P, EasyGCode's proven start/end G-code; the KUKA KR50 of the SPHR handover, its program's start and end ([printers.md](printers.md)). |
| G-code safety | Nothing printable unless golden-tested against EasyGCode or a hand-checked fixture. |
| Speeds | Always volumetric flow (mm³/s). |
| AI | Optional: tuning, exports and printing never need it. The owner's own key, never on the site ([api-key.md](api-key.md)). |
| Hosting | Public repo (all rights reserved, not open source), GitHub Pages, deployed from `main`. |
| In the app | Settings, results, warnings and errors only; explanations live in the docs. |

## Open

What's known to be missing or rough, by owner. When something here is done, delete it (and say
so in the dev log); when something new comes up, add it here, not only in the dev log.

### The app (general-manager)

- **STEP export:** lofts are written as triangles, not B-spline faces.
- **The lamp in the views:** the 3D view doesn't refit when the lamp comes apart (Fit does); the
  3D view doesn't highlight the shade selected in the Plan; while taking it apart each step takes
  the same time (a long thread spins faster), and turning shades aren't checked against the others.
- **The Plan:** a selected tube offers four pairs of add buttons (crowded on a phone until zoomed);
  add buttons check the new shade's straight start, not its body (a shade that runs into another
  shows red once added).
- **Prints overview:** its colours follow the assembly's height, not each print's.
- **Playback:** the progress slider steps by points, not by time.
- **A first-turn helper:** the closed end's disc works out `vase_revolve`'s first turn in the recipe
  (as the brim does); a step that gives a toolpath's first point would remove the copy.
- **The agent:** not tried on the Lamp yet; still to do: the chat box as a command line, skills,
  a test set of real requests.
- **Code health:** `viewport.js` (1,400 lines) and `app.js` (1,600) want splitting.
- **The robot (KUKA KR50):** not yet run on the controller (it must compile: `C_SPL` between
  SPLINE blocks, 1000 segments a block); reach, joint-limit and collision checks are a script
  (`scripts/robot-check.py`, on the handover's model), not in the app; the screw calibration (30
  per mm³/s) is from the handover's numbers, not measured; no travels or machine events on the
  robot; the print time estimate doesn't know the robot's speed cap.

### Recipes and projects (recipe-developer)

- **Overhang tuning** (tests 1–6, 14–15 Nov; the Bambu with a 0.8 mm nozzle, E14 (2 mm pitch),
  5 mm line, flow 15): the overhang settings only, layer-height compensation. **Every turn keeps
  the same plastic** (even layer lines); the compensation is only how close the turns sit, so the
  plastic squeezes. Never extrude less on overhangs. Found so far: test 5 (85 %, steep 0.8,
  pivot 70) printed well to about 70° and overflowed at 74°; its turns stepped out about half a
  bead (2.5 mm) each, which held. Test 6 (80 %, steep 0.8, pivot 76, opening 160 mm, so about
  79° at the rim) eases the top turns a little. Change one thing at a time (flow changed between
  tests 1 and 2).
- **The bell in-out** changes to the shade's layer and line at once at the throat (it has no
  fillet there): ease it over its first curve.
- **The Lampshade and the Lamp's shade** are two copies of the same steps; a change to one has to
  go to the other (or the Lampshade becomes a one-part Lamp).
- **Lamp:** `along` has a fixed range (it could follow the section's length); parts can't be
  renamed; a hanging lamp is the plan turned over; very thin shade layers (under ~0.4 mm, heavily
  squeezed) can stop with an error; `shade_layer`/`shade_line` have no Plan marks.
- **Marks** (tap in the Plan, jump to the setting): only the Lamp and the Lampshade have them.
- **Ready to use** (from the app, 15 Nov): `when … and …`, so the wings' settings can hang off
  the shape and each other; `toolpath(…, layers: n)` for the wings' layer count; a group of its
  own for the wings (the smoke test no longer lists every group). The bend stool's primer lines
  still count as layers (the stool shows 14 for 11): mark them `aid(…)` and they won't.
- **Wings:** check the tips' pace (½ line a layer) in a print; "same layer length" with deep
  outside wings flares hard (outside 200 %: 23 → 144 mm over the fade-out), maybe cap it; tips on
  a narrowing top still overhang a little.
- **Thread pendant** ([thread-pendant.md](thread-pendant.md)): a test print on E27; measure an E14
  barrel; maybe a pace for the wing rings (half a line a layer) instead of the max overhang; the
  floor lamp, linear pendant and screw part if wanted; Plan marks.
- **The coat hanger** as a template.
- **Tube with wings** ([tube-with-wings.md](tube-with-wings.md)), in the Lampshade and the Lamp:
  print it (the crease where a wing starts; "same layer length" within 4 %).
- **Overhang compensation: layer height only.** The owner uses only "layer height"; width
  compensation is no longer kept up (it stays where EasyGCode's golden tests need it, unchanged).
- **Tube with wings, STEP:** its shown wall (`wall`) is the plain tube, so its STEP export has no
  wings; show (and export) a surface with the wings if one is wanted.
- **Not printed yet:** two shades screwed together; a closed shade.

## Later, when wanted

- **Bead width from the layer below** (TOOLPATHS' "Auto Width"): use the bead simulation
  (`easycad_core::deposit`) while printing, so each move's extrusion fits the gap it actually
  has. Needs the simulation in the toolpath steps, a hand-checked fixture, and a cap on how far
  extrusion may move from the plan.
- **Robot-arm printing, more:** other arms (ABB RAPID, G-code with tool axes); reach checks in
  the engine (see Open, the robot).
- **Send to the printer** (Klipper/Moonraker; Bambu LAN later), probably through a small desktop
  helper.
- A project library on the device; presets. (List settings of points have a graph to drag
  them on, since 19 Nov; adding and removing points there, and other curve shapes, are open.)
- 3MF export; solids and booleans; Rhino `.3dm`, SVG/DXF.
- A multi-threaded engine (needs cross-origin isolation); Claude Desktop/MCP control.
