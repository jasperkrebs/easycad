// How the recipe language works, written for an AI (and readable by people). Used by the in-app
// agent's system prompt and by "Copy for AI". The list of steps is generated from the engine
// (nodeCatalog), so only the language itself is described here. Keep it in step with
// docs/definition-language.md.

export const LANGUAGE_GUIDE = `How the EasyCAD language works (easycad 0.1)

A recipe is a small dataflow program, like a Grasshopper definition written as text. The app runs it on every change and shows the result; the settings window is generated from its \`param\` lines.

Header
- The first line is \`easycad 0.1\`.
- \`project "Name"\`, \`printer "Bambu P1P 0.4"\` or \`printer "Ginger G1"\` (the pellet printer), and any number of \`note "…"\` lines (what the project is and why it's built this way: the project's memory, saved with the file).

Settings (each \`param\` is one control in the settings window)
- \`param height = 160 mm in 40..300 "Height"\`: a slider with a range, in the setting's own unit. Add \`step 1\` for whole numbers or fixed steps.
- \`param solid = true "Solid bottom"\`: a toggle. \`param style = "round" of ["round", "square"] "Style"\`: a dropdown. \`param label = "A" "Text"\`: a text field.
- \`group "Shape"\` puts the params below it into a group. \`@assumed\` after a param marks a guessed value; \`@hidden\` hides it from the window.
- Units: mm, cm, m, inch (lengths); deg, rad (angles); % (18 % means 0.18). Values without a unit are plain numbers (mm for lengths, degrees for angles).

Steps
- \`name = expression\` is one step; the name is its output wire. Steps can be in any order; a name can't depend on itself. There are no loops, ifs or user-defined functions.
- Calls take positional arguments first, then named ones: \`star(7, outer: 45, inner: 36)\`.
- Pipes: \`a |> f(x)\` means \`f(a, x)\`, and \`a |> f\` means \`f(a)\`. A line starting with \`|>\` continues the step above it.
- Arithmetic: + - * / ^ and parentheses; \`-t^2\` is \`-(t^2)\`. Use \`mod(a, b)\` for remainders (% is the percent unit). \`pi\` is 3.14159…; trigonometry takes degrees (sin(90) is 1).
- Steps are named after Grasshopper components in lower case (each step's description names the component it matches).
- Comments start with #.

Lists (Grasshopper's list matching)
- \`[a, b, c]\` and \`range(0, 1, count: 20)\` make lists.
- A step that receives a list where it expects one value runs once per item, so \`outline |> rotate(twist * t)\` with a list \`t\` makes one curve per value. With several lists, the longest one sets the count and shorter ones repeat their last item. Arithmetic on lists works item by item.
- Steps that take a whole list say so in their inputs (\`loft(sections: List<Curve>)\`). A list of lists works like a Grasshopper tree: such a step runs once per inner list; \`flatten\` makes one list.

Fields (numbers that vary over space)
- \`distance(curve)\` is a field: at each point, how far it is from the curve (negative inside a closed curve). \`fade(…)\` turns a distance into a 0..1 weight.
- Arithmetic and the number steps (min, clamp, sin, smoothstep, …) work on fields point by point, so \`line * (1 + near)\` is a field too.
- Inputs typed \`Length | Field\` accept a field and read it at every point they make (a spiral's spacing, a wave's amplitude). That's how one setting can affect only part of a shape, like an attractor in Grasshopper.
- \`x\`, \`y\` and \`z\` are fields: each point's own coordinates (a setting or step with that name wins). Given a field, \`move\`, \`rotate\` and \`scale\` work point by point: \`move(c, z: 0.1 * x)\` tilts a flat curve into a slope, \`rotate(wall, z * 0.5)\` twists 0.5° per mm, \`scale(wall, 1 + 0.002 * z)\` flares. \`sample(field, point)\` reads one value.

Geometry
- Millimetres, Z up. The origin is where the printer puts the print's centre (on the bed for z = 0).
- Points: \`point(x, y, z)\` or \`[x, y, z]\` (2 numbers give z = 0) anywhere a point or vector is asked for; \`[[0, 0], [10, 0]]\` is a list of points. Planes: \`plane(origin, normal)\`; move and rotate them like geometry (\`plane([0, 0, h]) |> rotate(a, axis: [0, 1, 0], center: [0, 0, h])\` is a plane tilted about Y).
- Curves are point lists, open or closed; \`circle\` and friends start on +X and run counter-clockwise. \`rotate\` takes any axis and centre (counter-clockwise looking against the axis). Surfaces are lofts through section curves (each section's first point is the seam); \`cap\` closes a loft into a solid. Toolpaths are nozzle moves with a line width, layer height and speed.
- Think in Grasshopper terms: build profiles from points and curves, transform them (by fields where each point needs its own amount), make layers with \`tween\`, \`contour\` or a lifted list, then print.

Outputs
- \`show a, b\` draws steps in the 3D view (otherwise the last step is drawn).
- \`print path\` is the toolpath the printer prints: the app writes G-code for the chosen printer from it.
- \`export shape as "name.stl"\` offers shapes (surfaces and solids, or a list of them) as an STL file; \`export path as "name.gcode"\` offers another toolpath as G-code. A recipe without \`print\` is plain geometry (show it and export an STL).

Printing
- Set print speed as volumetric flow: \`flow:\` in mm³/s on the toolpath steps (a \`param flow = 8 in 1..25 "Flow (mm³/s)"\` setting). Each move's speed then follows from its bead size (speed = flow ÷ bead area), so a thicker bead is printed slower. \`speed:\` in mm/s exists too, for when it's asked for.
- Vase mode: \`vase_spiral(wall, layer: …, width: …, flow: …)\` needs a loft of closed sections stacked upward (lift them with \`move(z: …)\`).
- Vase mode through any layers, tilted ones too: \`spiralize(layers, width: …, flow: …)\` takes closed layer curves from the bottom (on the bed) up, with matching points (make them with \`tween(bottom, top, range(0, 1, count: n + 1))\`, \`contour(wall, layer)\` or \`base |> move(z: …)\`). Each move is extruded for its own local layer height. Example, a cylinder cut by a tilted plane: \`top = base |> project_along(plane([0, 0, h]) |> rotate(angle, axis: [0, 1, 0], center: [0, 0, h]))\`, then \`layers = tween(base, top, range(0, 1, count: round(h / layer) + 1))\`.
- Line width about 1–2× the nozzle (0.4 mm on the P1P); layer height up to about 75 % of the line width; overhangs under about 45°; flow within what the printer melts (the P1P about 20 mm³/s with PLA).

EasyGCode techniques (every EasyGCode project is built from these; the thread connector and spoon templates show them at work)
- "0 means auto": \`pick(line, line, 2.4 * pitch)\`. A dropdown looked up in a table: \`item([2, 3, 4], index_of(["M2", "M3", "M4"], size))\`. Masks for zones: \`greater(z, 50)\`, \`less(…)\` (1 or 0).
- Threads and coils: \`helix(circle(r, tolerance: 0.01) |> rotate(90), pitch: …, height: …, width: …, flow: …)\` rises exactly one pitch per turn.
- Bottoms and seats: \`rings(outline, line)\` (or \`inset: true\` for outlines that aren't round), \`reverse\`, then \`link_rings(…, "staircase" | "zipper" | "spiral", line)\` makes one line for \`toolpath\`.
- Brims and other parts: \`join([brim, wall], clear: 2 * brim_layer)\` hops over the brim; parts that meet end to end carry straight on. \`stack(layer, n, lift: 0)\` travels straight over at the next layer. \`center(…)\` puts things on the bed's centre (after \`rotate\` for a diagonal fit). \`bead(path, width:, layer:)\` extrudes part of a print for another bead without moving it.
- Tuning a toolpath by zone (fields of x, y, z, \`distance\`, \`lean(wall)\`): \`adjust(path, speed: …, extrude: …)\`, \`fan(path, level: …)\`, \`dwell(path, points, seconds)\`, \`accelerate(path, mm/s²)\`, \`spikes(path, scatter(…) mapped onto the wall, width:, length:)\`. Overhang-paced layers: \`contour(wall, layer, pace: 1)\` then \`spiralize\`; an open rim: \`flat_top: false\`.
- Machine events on a toolpath, before it prints (after the travel to it) or with \`at_end: true\` after it: \`temperature(path, up:, mid:, down:)\` (pellet zones; \`nozzle:\` on a filament printer; nothing given = back to the printer's own), \`wait_temperature(path, …, cool: true|false)\` (put it on a primer line), \`speed_override(path, %)\` and \`extrusion_override(path, %)\` (M220/M221). Foaming (the bend stool template): heat after the first layer, wait on a primer at the bed's corner, extrusion 38 % with speed 100 × 100 ÷ 38 %, then back to 100 %, cool, wait on a second primer; \`join([...], clear: 2 × the tallest layer so far)\` hops to the primers. Layers that wipe across: \`stack(layer, n, wipe: 2 * layer)\`. Rings with radial steps: \`arc(radii, start:, end:, tolerance:)\`, then \`polyline(flatten(vertices(rings)))\`.
- Round shades and vases: a profile (radius x against height z, drawn in the XZ plane) printed by \`vase_revolve(profile, layer:, width:, flow:)\`; layer, width and flow may be fields of z, read once per turn (overhang compensation: \`width: line * min(2.5, 1 / max(0.05, cos(abs(graph(heights, angles, z)))))\`). The lampshade template builds EasyGCode's shades (cone, bell, dome, sphere, flat sphere) as profiles; a new shade is a new profile (the \`own_shade\` step). \`graft(list)\` makes a whole-list step run once per item (a toolpath per brim ring, for \`join\` to hop between).
- Vessels (the vessel template): sections = \`tween(bottom_shape, top_shape, t) |> scale(x: s, y: s) |> move(z: …)\` with the scale from a profile (\`graph\` of a \`catmull_rom\`), \`loft\`, then \`contour(wall, layer, pace: 1)\` (closer layers on overhangs) and \`spiralize(…) |> bead(layer: layer)\` (every turn extruded for a full layer: the squeeze). A non-planar rim: \`contour(…) |> move(z: lift field)\`. A bottom that rounds into the wall: a flat spiral (\`link_rings(reverse(rings(flat, line)), "spiral", line)\`), then fillet sections along a quarter circle in the same loft, \`spiralize(…, ramp: false)\`, joined into one line.`;
