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
  diameter less the engagement, plus half a line). Every point's height comes from its angle round
  the axis (**heights by angle**), so every ring is a true helix one pitch a turn and the slots
  take up the difference: the shade screws onto the socket. (Heights by length, as a slicer does
  it, drift: it warns.)
- **Above the grip:** only a crease at each entry (the lines on the outside run on to the top), or
  full wings leaning back out.

Within a layer the line goes down a little where a wing runs back round (its height is its
angle's): every point is still exactly a layer above the one under it.

## How it's built

One engine step, `thread_shade` (`crates/core/src/pendant.rs`), a port of the sketcher's
generator; the template (`web/templates/thread-pendant.easycad`) gives it the settings, prints it
with `toolpath` and `bead`, and says what's wrong from the numbers it returns (`output: "info"`).
A step for one product goes against "general steps, not per-project code" on purpose (the
owner's call): it keeps the sketcher's files and this template the same shade, which a recipe
rebuilt from general steps couldn't promise. A test checks it against points the sketcher itself
drew for two of its files.

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
| `eSock`, `grip`, `hide`, `topMode`, `crease`, `zMode`, `res` | Thread engagement, Grip height, Grip depth below the top, Above the grip, Crease depth, Heights by, Resolution |

Not made here (the file still opens, and says so): the **floor lamp** and the **linear
pendant**, the **separate screw part** (`mount: coupler`). The view-only settings (bulb, section
cut, the globe helper) aren't needed: the shape is its points.

## Printing it

What the sketcher leaves out, added here:

- **The first layer sits on the bed:** the sketcher's first layer rises off the bed by a whole
  layer as it goes round; here the nozzle starts on the bed and each move is extruded for the gap
  under it (`bead`), as a vase's first turn.
- **Overhang compensation by layer height**, as the Lampshade's (strength, steep curve, pivot):
  layers come closer where the wall leans, each still extruded for a whole layer. Never inside
  the thread: from a layer below the grip to the top every layer is exactly a pitch, so the grip
  stays a true helix (tested). A sphere's bottom, leaning 70°, gets layers down to about a third
  of the pitch.
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
