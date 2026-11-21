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
- **Entry points:** at each, the line turns in, and two **wings** run off the slot, one each
  way, as a loop: out along a middle ring, round a half circle onto an inner ring, back past the
  slot, round onto the middle ring, back to the slot.
- **Wing shape** (since 19 Nov), one of two:
  - **leaf** (the default): the wings as drawn. The **leaf** graph sets how wide they are (0..1
    of the room between entries) against height along the wing zone, exactly; they start at the
    wall where the zone starts and come in **in a straight line to the socket** (with full wings
    above the grip, back out to the wall at the top in another). Nothing is held back by a max
    overhang: judge the overhangs by eye (the half-a-line check still says where the wings step
    more than half a line a layer).
  - **sketcher**: the sketcher's wings (sketcher files open this way). Up the wing zone it's
    first a **crease**, getting deeper by at most the max overhang a layer, until it reaches the
    wings; the wing curve sets how long they are, slowed to the max overhang; below the grip they
    lean in over the path height along the path curve, never faster than the max overhang.
- **The grip:** the inner rings, all the entries' together, sit on the socket's barrel (its
  diameter less the engagement, plus half a line), as a true helix one pitch a turn: the shade
  screws onto the socket.
- **Above the grip:** only a crease at each entry (the lines on the outside run on to the top), or
  full wings leaning back out.

**Layers** (`z_by`):
- **angle** (the default since 19 Nov, the sketcher's): every point at the height its angle round
  the axis gives, so every layer rises one layer a turn, a vase spiral with no seam, and every
  ring is a helix (the thread on the socket one pitch a turn); the slots take up the difference
  (within a layer the line dips a little where a wing runs back round). Tested: along the outer
  wall the height never changes faster than a layer a turn, and a whole turn rises exactly its
  layer.
- **flat**: every layer level, except the thread (the inner rings in the grip climb a pitch a
  turn, easing in and out over 4 layers). Each layer steps up to the next along its last two line
  widths at the seam (between two entries): in the owner's prints that step is a seam all the
  way up the wall, too visible, so it's no longer the default (17–19 Nov it was).
- **length**: rising by the length so far, as a slicer's vase mode: the thread drifts (it warns).

A sketcher file's "angle" opens as angle.

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
| `eSock`, `grip`, `hide`, `topMode`, `crease`, `zMode`, `res` | Thread engagement, Grip height, Grip depth below the top, Above the grip, Crease depth, Layers, Resolution |

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

### The wings compensated on their own

With compensation on, the wall and the wings are compensated each at its own rate (since 20
Nov): the wall for the shape's lean, the wings for theirs in towards the socket (radially; not
the leaf's edges growing along the rings). Each layer is still one line, so where one rises more
than the other the wings' heights part from the wall's, and the arm between them takes up the
difference, its angle changing layer by layer: on a sphere, where the wall stands upright at the
equator and the wings lean in, the wings fall behind; where the wall leans more than them (the
top), it catches up.

- The arm blends the wings' heights into the wall's evenly along its length (a straight climb
  from the middle ring out to inside the wall), except where it leaves the wall: over the arm's
  outer 30 % it starts level and curves into the climb (no corner there; the rest a little
  steeper for it). Each layer's arm is its own curve, so it doesn't sit exactly on the one
  below; each point is extruded for the gap it's in. A crease above the grip follows the same
  blend, so it sits on the arm under it.
- Every point follows the shade's profile at the wall's height, not its own (on a sphere the
  shape lower down is wider: an arm far below the wall was pushed out against it, dropping
  almost straight down there, and the crease above it floated, a lip; fixed 20 Nov).
- Both have the same number of layers, so if the wings fell so far behind that they couldn't
  finish the grip before the wall reaches the top, the difference is scaled back (each wing
  layer between its own and the wall's), as little as it takes.
- With a crease above the grip, the difference left closes there faster: as fast as leaves each
  crease layer at least 75 % of its height.
- Where the wings move in while far below the wall's heights, the arm just outside the middle
  ring sits a little high on the one below (a straight arm whose low end moves in): each move is
  extruded for the gap it's in, and a check warns where that's over 1.5 times its layer. So does
  the one for the arm steeper than 15°.
- **On the owner's Sphere20test** (its sketcher wings, 70°): the wall keeps a whole 2.5 mm layer
  at the equator (before, the wings' 2 mm), the wings their own ~2 mm; the wings end some 25 mm
  below the wall's heights; 147 layers, 37 h.

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
- **The bridge is the straight arm.** In each layer the heights blend by radius along the arm
  only: all of the thread's from the middle ring on the socket inwards (in the grip both rings
  and the turns at their tips print the thread's height and bead, 5 × 2.5 mm), none from a line
  width inside the outer wall outwards, smoothly between. The arm starts at the same radius in
  every layer, so each point sits on the same point of the layer below even where the rings
  move (below the grip they're still wall layers). The bead blends the same way (each point
  extruded for its own width and layer height), and the two rings always have the same width.
- **On the owner's straight shade** (72.5 mm, a 3.5 × 1.5 mm wall, a 30 mm grip): the thread
  runs 9.5 mm below to 9.5 mm above the wall, the arm climbs at most 19° (17° when the blend ran
  from the socket ring), and the rings' thinnest layer is 1.13 mm.
- **Two checks** say when it goes too far: the bridge climbing steeper than 15° (the nozzle may
  drag on the layer below; a narrow shade round a long grip does it), and the wing rings' layers
  thinner than half the wall's (rings moving while the lead changes; full wings with thin
  layers do it).
- Without compensation and with the thread's own bead everywhere, nothing moves: one layer
  height, as before.
- **The gap between the wing lines** ("Air gap between wing lines") opens from none where the
  wings start: the first turn at each tip is then one line across and hangs at most half a line
  off the slot under it (with the whole gap it hung a full line, in the owner's photo). It opens
  a layer at a time by what's left of half a line (or the max overhang, if less) after the inner
  ring's own move that layer, so the middle ring never steps out further. **No gap on the socket**
  (on by default, "closed pockets"): it closes again before the grip, as fast as the max overhang
  allows on top of the inner ring's move, and only opens as far as it can close in time, so every
  layer on the socket is solid and each gap is a closed pocket. On the owner's Angled cone (70°)
  the 5 mm gap holds along the whole path and closes over the last three layers before the
  socket. Tested: the first turn a line across, the steps, no gap on the socket. With the leaf
  (since 19 Nov) the middle ring, the one the arm runs into, keeps to a straight line from the
  wall to its place on the socket and the inner ring sits the gap inside it: the gap opens and
  closes by the inner ring moving, so before the socket it comes in more slowly rather than the
  middle ring swinging in onto it (on the owner's half sphere its biggest step went from 5.6 to
  3.5 mm).
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

### On the robot arm

The **Robot** settings print it on the SPHR handover's KUKA arm ([printers.md](printers.md)):
the toolpath is the same (overhang compensation by layer height steps each wall layer one layer
along the wall), and `tilt` holds the nozzle along the wall: at most **Most nozzle tilt** (45°)
from upright, averaged over **Tilt smoothed over** (20 mm of line, so the wrist turns gently where
the line runs in from the wall to the wings and back), and growing from upright on the bed to
the most at **Full tilt from this high** (60 mm: tilted 45°, the tool's fans hang about 42 mm
below the nozzle; 60 keeps them 18 mm off the bed, `scripts/robot-check.py` checks it). Each move then gets its full bead back along the nozzle. Choose the KUKA KR50 in
the Print tab; its **Stop at this height** makes a short test. The owner's Sphere on the robot is
`fixtures/recipes/sphere-robot.easycad` (60 mm³/s: about 1800 screw command, the handover's own
wall at 6 mm/s; the first 40 mm take about an hour).

## Open

- On the robot: a test print, and the arm's reach and tool clearance over the sphere's closing
  top (not checked in EasyCAD yet).
- A test print, E27 (the E14 barrel is the sketcher's placeholder, 28 mm: measure one).
- The wing rings' step: the check says when; maybe a pace like the Wings' (at most half a line a
  layer) instead of the max overhang, for the wings only.
- The floor lamp (its rim openings thin the last layers), the linear pendant and the separate
  screw part, if wanted.
- Plan marks (tap the shade, get its settings) and the bulb's clearance check.
