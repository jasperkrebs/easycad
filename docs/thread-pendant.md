# Thread pendant

*16 Nov 2026. A friend of the owner's built the **Thread Lampshade Sketcher**, a Claude
artifact that designs lampshades in the spirit of the Wings, with more settings. It draws and
exports STLs; it doesn't print. The owner asked for its pendant here, printable, reading its
settings files natively, threading onto an E27 socket (E14 later).*

The **Thread pendant** template is that pendant: the same shade, point for point, printed as one
line, with what printing needs on top. Open a sketcher settings file (`.json`) with **Open** or
**Load settings** and it becomes a Thread pendant with its values; **Save for the sketcher**
(in Settings, on a Thread pendant) writes one back that the sketcher opens.

## The shade

Printed as it hangs: the bottom opening on the bed, the socket going in at the top.

- **The outer wall** follows the shape: a curve through your points (radius to the bead's
  middle against height 0..1), a monotone cubic that never overshoots them, with a corner where a
  point is marked as one.
- **Entry points:** at each, the line turns in. Up the wing zone it's first a **crease**, a
  radial slot getting deeper by at most the max overhang a layer. Once the slot reaches the wings'
  radius, two **wings** run off it, one each way, as a loop: out along a middle ring, round a half
  circle onto an inner ring, back past the slot, round onto the middle ring, back to the slot. The
  wing curve sets how long they are along the zone, slowed to the max overhang.
- **The path to the socket:** below the grip the wings lean in from the wall to the socket's
  barrel over the path height, along your path curve, never faster than the max overhang.
- **The grip:** the inner rings, all the entries' together, sit on the socket's barrel (its
  diameter less the engagement, plus half a line), as a true helix one pitch a turn: the shade
  screws onto the socket.
- **Above the grip:** only a crease at each entry (the lines on the outside run on to the top), or
  full wings leaning back out.

**Layers** (`z_by`, since 17 Nov):
- **flat** (the default): every layer is level, except the thread. The inner rings in the grip
  climb a pitch a turn (a pitch below the layer at the seam, level with it a turn on), easing in
  over the 4 layers below the grip and out over the 4 above (so the layers next to it still sit
  on it); the half turns onto and off them climb with their ends. Each layer steps up to the next
  along its last two line widths, at the seam (between two entries).
- **angle**: the sketcher's: every point at the height its angle round the axis gives, so every
  ring is a helix and the slots take up the difference (within a layer the line dips a little
  where a wing runs back round).
- **length**: rising by the length so far, as a slicer's vase mode: the thread drifts (it warns).

A sketcher file's "angle" opens as flat: what it was for, a true-helix thread, is kept.

**The slots land on the rings:** the sketcher draws each slot straight in its own frame, so its
ends land a hair outside the wall and the rings (0.2–0.4 mm, most near the socket), a little
zigzag where each ring starts (the owner's photos, 17 Nov). Here they land exactly, so the
sketcher's points are within half a millimetre of these (tested), not the same.

## How it's built

One engine step, `thread_shade` (`crates/core/src/pendant.rs`), a port of the sketcher's
generator; the template (`web/templates/thread-pendant.easycad`) gives it the settings, prints it
with `toolpath` and `bead`, and says what's wrong from the numbers it returns (`output: "info"`).
A step for one product goes against "general steps, not per-project code" on purpose (the
owner's call): it keeps the sketcher's files and this template the same shade, which a recipe
rebuilt from general steps couldn't promise. A test checks it against points the sketcher itself
drew for two of its files (within half a millimetre: the slots' ends, above).

The translator (`web/js/sketcher.js`, tested with the sketcher's own files in
`fixtures/sketcher/`):

| Sketcher | Thread pendant |
|---|---|
| `profile` (u, r, sharp) × `scale` | Shape: [height, radius, 1 for a corner] |
| `H` | Height |
| `wing` (u, l), `wingLen`, `N`, `maxOver` | Wing length along the zone, Wing zone, Entry points, Max overhang |
| `tipGap`, `neckGap`, `wallGap`, `pinGap`, `wingRot` | Gap at the wing tips, Entry slot width, Air gaps, Wing rotation |
| `path` (u, w), `pathLen` | Path, Path height |
| `socket`, `sockD`, `sockP` (= `LH`), `sockH` | Socket (E27, E14, or custom with the three) |
| `eSock`, `grip`, `hide`, `topMode`, `crease`, `zMode`, `res` | Thread engagement, Grip height, Grip depth below the top, Above the grip, Crease depth, Layers ("angle" opens as flat), Resolution |

Not made here (the file still opens, and says so): the **floor lamp** and the **linear
pendant**, the **separate screw part** (`mount: coupler`). The view-only settings (bulb, section
cut, the globe helper) aren't needed: the shape is its points.

## Printing it

What the sketcher leaves out, added here:

- **The first layer sits on the bed:** the sketcher's first layer rises off the bed by a whole
  layer as it goes round; here it's flat on the bed (by angle or length: it starts on the bed and
  each move is extruded for the gap under it, as a vase's first turn).
- **Overhang compensation by layer height**, as the Lampshade's (strength, steep curve, pivot):
  layers come closer where the wall leans, each still extruded for a whole layer. A sphere's
  bottom, leaning 70°, gets layers down to about a third of the pitch. **Only on the outer
  wall:** it's compensated in every layer, bottom to top, the grip's included, while the thread
  keeps its pitch.
- **A thinner wall and wings than the thread** (Print: "Wall and wings: line width" and "layer
  height", 0 for the thread's): everything but the thread prints with its own bead, the thread
  with the pitch and `line`. Plastic goes with the line width, not the layer height: the owner's
  Angled cone with 3.2 mm lines and 1.5 mm layers takes 2,004 cm³ and 28 h, against 2,980 cm³
  and 41 h. The Ginger G1's width check passes 1–2 × the nozzle, so a 3.2 mm wall wants the 3 mm
  nozzle (and a 5 mm thread on it is 1.7 ×).

### How the thread keeps its pitch

The wall (and with a thinner wall, the wings) rises less than a pitch a layer; the thread on the
socket rises exactly a pitch. So in the grip the thread pulls ahead of the wall, a little more
every layer. Each layer is still one line, so each layer climbs from the wall's height to the
thread's and back: through the slot and along the wing rings (the bridge).

- **The thread's own heights.** Below the grip the inner ring is a wall layer. Over the layers
  before the grip its rise grows a step at a time from the wall's to a pitch (a tenth of a pitch
  a layer, over at least 4 layers), its width and bead with it. On the socket, and the layer
  under it, it rises exactly a pitch: a true helix (tested, flat and by angle, and on the owner's
  shade through the whole toolpath). Above the grip it eases back over as many layers.
- **The lead is shared.** The thread starts half its lead below the wall and ends half above it,
  so the bridge climbs half as far. To start low, the wing rings rise less than the wall for a
  while below the grip; above it, they rise less again until the wall has caught up (as far as
  there are layers). Never less than 75 % of the wall's rise, easing in and out of that a
  twentieth of it a layer. With full wings above the grip (they move back out, and would lose
  the lead they carry) all of the lead is below.
- **The bridge.** In each layer the heights blend by radius: all of the thread's on the socket's
  inner ring, none from a line width inside the outer wall outwards, smoothly between. Each point
  sits on the same point of the layer below. The bead blends the same way (each point extruded
  for its own width and layer height), so every ring is as wide as its layer is tall: the middle
  ring next to the thread prints nearly as the thread.
- **On the owner's Angled cone:** with compensation only (2.13 mm wall layers) the thread runs
  3.3 mm below to 3.3 mm above the wall, the bridge climbs at most 3.5°, and the rings' thinnest
  layer is 1.76 mm. With a 3.2 × 1.5 mm wall: ±11.6 mm, 12.5°, 0.84 mm.
- **Two checks** say when it goes too far: the bridge climbing steeper than 15° (the nozzle may
  drag on the layer below; a narrow shade round a long grip does it), and the wing rings' layers
  thinner than half the wall's (rings moving while the lead changes; full wings with thin
  layers do it).
- Without compensation and with the thread's own bead everywhere, nothing moves: one layer
  height, as before.
- **Line width and flow** are settings (the sketcher's bead is 5 mm), and the G-code comes from
  the printer profiles like every other project.
- **The checks:** the sketcher's (wings not formed over the grip, the socket showing, the wings
  touching the socket body above the grip, too little room for fingers at the top, the path or the
  wing curve slowed, the wall pushed out, too many entries, heights by length), and one more: **the
  wings step more than half a line a layer**. The max overhang lets the wing rings move in or out
  by pitch × tan(max overhang) a layer: at 70° that's 6.9 mm, more than a 5 mm line, so the new ring
  has nothing under it. Of the friend's four files, the sphere, the angled cone and the half sphere
  (65–70°) do that; at 45° they don't, but a wide shade then needs a longer wing zone and path to
  bring the wings in to the socket.

## Open

- A test print, E27 (the E14 barrel is the sketcher's placeholder, 28 mm: measure one).
- The wing rings' step: the check says when; maybe a pace like the Wings' (at most half a line a
  layer) instead of the max overhang, for the wings only.
- The floor lamp (its rim openings thin the last layers), the linear pendant and the separate
  screw part, if wanted.
- Plan marks (tap the shade, get its settings) and the bulb's clearance check.
