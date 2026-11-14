# The lamp builder: shades that screw into each other

The owner's idea (8 Nov 2026), read back and agreed: every vase-mode wall is a thread on both
faces (one bead a turn, one pitch a turn). So a shade can screw onto the outside, or into the
inside, of another shade's straight section, going up or down, and the new shade's own
straight ends take more shades in turn: a sandbox of shades, planned on screen, each printed on
its own, without asking the AI.

## Decisions (8 Nov 2026)

| Question | Decision |
|---|---|
| Pitch | One for the project: the layer height, which is the socket's thread pitch (E27 2.5 mm, E14 2 mm, or the custom socket's). |
| Line width | One for the project. |
| Fit | Two settings: the socket's thread, and every shade-to-shade thread. |
| Joint length | Per shade: its straight start (the holder that screws on) and its straight end. A global setting overrides every joint's length. |
| Up and down | As printed: z up from the main shade's throat on the bed. |
| The first shade | On the socket, or on a custom thread (a tube with no socket: the custom socket). |
| What connects | A shade's straight start and straight end, inside and outside. A long tube can hold several shades, each sitting further along it. |
| Assembly order | Not checked. Only collisions are. |
| Printing | Each shade is its own G-code file, printed alone in the middle of the bed. |
| New shapes | Tube, bell in-out (opens and closes again; optionally straight for a while at its widest, `widest_len`), cone in-out (straight lines out and in; the same straight band, and its corners at the widest rounded, `widest_fillet`). |

## How it fits the thread

As the salt shaker's lid (which threads in practice): a shade on the outside of another's
straight section runs its nozzle `line + fit` further out; one on the inside, `line + fit`
further in. Both are printed one bead a turn at the same pitch, and the same way round, so the
threads match; a shade printed the other way up is the same helix turned over, still the same
hand. On the socket, as today: `(thread diameter + socket fit + line) / 2`.

## How it's built

### 1. Components and parts in the recipe language

A project holds one **component** (a recipe within the recipe: settings, steps, outputs) and
any number of **parts** made from it, each with its own settings:

```
component shade "Shade"
  param shape = "cone" of ["tube", "cone", ...] "Shape" @ask
  input mount = socket "Mounted on"
  ...steps...
  port end_out = [...] "End, outside"
  print place(path, z: z0, flip: down)
  show placed_wall
end

part main = shade()
part tube = shade(mount: main.end_out, direction: "down", shape: "tube", length: 300 mm)
```

- A part's settings are the component's settings, named `part.setting` (`tube.shape`), with
  the values given in its line; the rest keep the component's defaults. Moving a slider writes
  the value into the part's line (or adds it there), touching nothing else.
- An **input** is a setting whose value is another part's **port** (`main.end_out`); the
  settings window shows it as a list of the ports there are.
- A **port** is a named output, a place another part can be mounted, with a label. For the
  lamp it's `[radius, height, inward, length, kind]`: the radius the next shade's straight
  section runs at (the thread's fit already in it), where the straight section ends, which way
  it runs from there, how long it is (0: no port), and whether it's the socket.
- Every name inside the component is the part's own; settings and steps outside it are shared
  (the socket, the layer height, the line width, the fits, the flow).
- The engine expands the parts into one ordinary recipe before it runs, so everything else
  (steps, errors, the settings window, printing, the agent's edits) works as before.
- `@ask` settings are the choices a new part comes with (the shape, which way it goes): in the
  Plan a `direction` choice is one add button per way, the rest start at their defaults and are
  on the selected part's bar.

Commands (the same for the buttons, the agent and the CLI): `addPart(component, name, values)`
writes a new `part` line; `removePart(name)` removes it and the parts mounted on it.

### 2. Placed toolpaths and the collision check

- `place(path, z, flip)` puts a part where it goes in the assembly, for the view only: the
  G-code stays the part as printed, on the bed. Flipped is turned over about X (the same hand
  of thread, not a mirror image).
- Each shade exports its wall as a STEP solid, `<part>-shade.step` (`thicken(wall, line)`),
  where it goes in the lamp, for a manufacturer who slices it their own way. The thread is
  printed by the bead, not modelled: the file's header lists the socket and settings.
- The engine checks every pair of placed parts for beads that run into each other (closer than
  three quarters of the two beads' widths, at the same height), and warns with the parts'
  names and the height. Shades screwed together sit a line plus the fit apart, so they pass;
  a tighter fit than a quarter line would be flagged.

### 3. The shade component

The lampshade's profile code, generalised:
- the start is a straight holder at the mount's radius (the throat on the socket), its length
  the shade's start length or the global joint length;
- shapes: tube, cone, bell, dome, sphere, flat sphere, bell in-out, cone in-out, own;
- an optional straight end for every shape (the spheres' lip, for all), its length the shade's
  end length or the global joint length;
- where it sits: on a port, going up or down, and how far along the straight section it sits
  (0: flush with its end);
- ports: start inside, start outside, end inside, end outside (the start's inside is taken
  when the shade is on the socket; the end's only when there's a straight end).

The single-shade lampshade template has the same shade arithmetic (since 12 Nov 2026: every
shape, a lip on any shape, the closed end), with its own setting names (`throat`, `lip`), and
stays golden-tested against EasyGCode.
The lamp builder is a new template, **Lamp**.

### 4. The app

- **Settings:** the shared groups as today (Socket, Print, Overhangs, Brim, Joints), then one
  fold-out per shade, headed by its name and shape, with a remove button. Inside: where it's
  mounted, which way, how far along, its shape and that shape's settings, its start and end.
  "Add shade" shows the Plan, with the last shade selected.
- **Plan** (a fourth view next to Shape, Toolpath, Both; the place to work on the assembly):
  the assembly's cross-section, every shade a line as wide as its bead, in its colour (its
  custom colour when chosen; red where it runs into another; faint while hidden from the 3D
  view), its name on the left. Drawn in screen pixels: lines, labels and buttons keep their
  size; the wheel or a pinch zooms, a drag pans, a double-click or Fit fits it again.
  - **Tap a shade** to select it: a halo, the others step back, and its **bar** along the plan's
    foot: its name (its settings), its shape (a list), which way it goes (an arrow: tap to
    flip), settings, remove, its on/off settings that apply (a straight end, printed wide edge
    down) and done. Tap it again, tap nothing, or Escape, to deselect. With the Settings tab
    showing, its fold-out comes into sight.
  - **Add buttons:** beside every free port of the selected shade (of the only shade, when
    there's one), one button per way a new shade can go: a `+` with an arrow up or down (the
    component's `direction` choices), on the side of the wall it screws onto (outside: to the
    wall's right; inside: to its left, towards the axis), with a stem to the spot. One tap adds
    the shade (`addPart`) with the component's default shape, its sizes scaled to where it goes,
    and it arrives selected, its own add buttons showing, so the next one is one tap more. The
    other shades' free ports are dots. Buttons are 40 px, pushed apart along the wall where
    they'd overlap.
  - **Which ports are free** comes from the drawing: a port with a straight section, and room on
    its face for a straight start (20 mm, or the section's length) that no shade's beads lie on.
    Where one already sits (a shade at a tube's end), the new one goes further along (`along`)
    past it; a short section that's taken offers nothing.
  - **Just added**, while nothing else has changed, a new shape or way from the bar adds the
    shade afresh (from the recipe as it was before): a `cone` switched to a tube is called
    `tube`, with the tube's sizes. After that a new shape keeps the name, and brings its sizes
    for the settings the old shape didn't use.
  - **Tap a mark** (start, fillet, shade, opening, height, straight end…): its shade is selected
    and the Settings tab opens at the setting, lit up.
  - **Zoom** with a pinch (Safari's own gesture too), the wheel, or the `+` and `−` buttons at
    the plan's top right. A press that wanders less than 12 px (5 px with a mouse) is a tap.
  - **A brim** (`aid`) prints and shows in 3D, but isn't in the Plan: it's there to print safely,
    so it takes no face and runs into nothing.
  - **Taken apart:** the assembly row (under the print's, also in the Plan) plays the lamp
    apart a shade at a time, the furthest out first: each turns off its thread as a right-hand
    thread does (one pitch a turn, the pitch read from its toolpath) and then moves on, without
    turning, until the whole shade (and whatever already came off it) stands clear of every shade
    it would overlap round the axis and in height, with a space as long as its thread between them
    (at least that far where nothing's in the way). Only the shade coming off turns: whatever is
    already off keeps the angle it came off at and only rides along with the shade it was on. **Exploded** jumps to the end; played from there it goes back together.
    The Plan shows the shades where they are (no add buttons while it's apart). A view only.
    (Dragging a shade along its thread, 10 Nov, was dropped: on a phone it fought the pan.)
  - With no shade at all, one add button (up) puts the first on the socket.
- A setting changed anywhere (the bar, a slider) writes only that setting into the part's line.
- **Shade layers** (12 Nov 2026; the Print group's `shade_layer` and `shade_line`, 0: the pitch
  and the line): the threads print at the pitch and the line, the shades between them at their
  own layer height and line width, for looks. Through the start's fillet the layer and line
  ease along an S-curve of height (`smoothstep`) from the thread's to the shade's, and with a
  straight end back again through the end's fillet, so the end prints at the pitch and other
  shades screw onto it; with no fillet the change is at once, where the straight section ends
  (or starts). A tube is all thread. Overhang compensation works on the local values: a
  layer-height-compensated turn rises `layer × squeeze` and is extruded for the full local
  layer (`vase_revolve`'s `bead_layer` as a field); width compensation widens the local line.
  Along the surface the same, with heights read along the profile. The settings are the
  project's (as the pitch, the line and the flows are), not each shade's: one lamp, one look.
  Ports are unchanged: the threads they fit stay at the line.
- **Closed end** (a shade's tick box, 12 Nov 2026): its end shut by a flat disc (its own layer
  height, the shade's layer by default, and the shade's line) rising through a fillet with its own radius into the wall,
  printed disc down: the disc as a spiral from the middle out, a line a turn, turned so it ends
  exactly on the wall's first point (no step out onto the fillet), then the shade
  as every shade prints (`vase_revolve`, overhang compensation and all), its profile from the
  disc's edge up the fillet (flat at first, so compensated the most) and the body to the
  start's thread, a pitch a turn on the thread; the step from the disc onto the wall is
  printed (`join(link: true)`), no hop. Shown where it threads; its closed end has no ports;
  the disc and its fillet are marks in the Plan.
- **Prints** (next to Plan, for a project of several parts): every part laid as it prints, each
  on its own bed outline (the printer's bed, its origin where the printer puts it), side by
  side, like a slicer's plates; a card per part: which way up (Flip: its wide edge down, or
  start down; a closed one is "Disc down", always) and its G-code.
- **3D view:** the whole assembly, each shade where it goes; the parts buttons show one shade.
- **Print:** one G-code per shade, each on the bed's centre.

## Phases

All built 8 Nov 2026:
1. Components and parts in the language (parse, expand, rewrites, commands), with tests
   (`crates/lang/src/tests.rs`).
2. `place` and the collision check (`crates/core/src/assembly.rs`, `eval_tests.rs`).
3. The shade component and the Lamp template (`web/templates/lamp.easycad`). A one-shade lamp
   prints EasyGCode's lampshade G-code, all four golden fixtures (`golden_tests.rs`); the
   assembly tests are in `lamp_tests.rs`.
4. The app: settings per part, add and remove, the Plan view (`web/js/parts.js`); the smoke test
   adds a tube on the plan and removes it. 10 Nov 2026: the Plan's add buttons (a `+` and an
   arrow per way, on the face's side), selection and the bar, zoom and pan; the smoke test adds
   shades through the buttons (outside and inside), switches a shape and flips a shade from the
   bar, removes one, selects and deselects, and checks a hidden shade; `settings.test.mjs` checks
   which ports are free and how far along.
5. Docs and the agent's guide.

## Still open

On the roadmap's "Open" list ([roadmap.md](roadmap.md)): the Plan and the lamp's views under
"The app", the shade's own under "Recipes and projects".
