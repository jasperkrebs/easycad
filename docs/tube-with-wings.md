# Wings (Lampshade and Lamp)

*The owner's idea (14 Nov 2026), from a sketch; first the Lampshade's shape "tube with wings",
then the Lamp's too, then "cone with wings" and the rest; since 15 Nov a setting of every shape
(**Wings**, at the end of the Shade group, like the brim: on, and its own "Wings" group shows),
following one of three things (`wing_mode`):*

| Wings follow | The wings | The shape |
|---|---|---|
| **fades** | the Wings %, faded in (or full from the bed, the tube) and out, linear or smooth | as set; a tube can widen as they fade out (below) |
| **curve** | a smooth curve through your points: where along the shade (0 where the wings can start, 1 where they end) and the % there, e.g. 0 → 100 %, 0.6 → 80 %, 1 → 20 %; moving a point moves where the curve is steepest (no other fades) | as set |
| **fit the shape** | as much as keeps every layer as long as the shade's widest (at most the Wings %), faded in from the throat | as set |

*Everything else is the same in every mode: inside or outside, seams, gap, how it rises, the
straight start, wide edge down, where a thread can go (the Lamp). Below, "the tube" is the tube
shape with wings on; the other shapes have their own radius at every height, and the wings sit
on that wall.*

## Why

A narrow tube prints short layers: each one is done before the one below has cooled, and the
wall slumps. Wings make every layer longer **without making the tube any wider**: at the seam
the line turns back on itself, so each layer has more to print (more time to cool) while the
tube stays the same.

## The path, one layer

Starting on the tube's wall and going round (counter-clockwise from above, like the spiral):

1. the tube's wall, round to the seam;
2. a half-circle turn **in** onto a middle wall, then **back** against the print direction (the
   first wing);
3. another turn in onto an inner wall, **forward** through the seam and past it as far again;
4. a turn **out** onto the middle wall, back to the seam (the second wing);
5. a turn out onto the tube's wall, and on round: the next layer.

So the inner wall is one arc through the seam and the middle wall two arcs, one on either side
of it. The walls sit a line apart (plus the **gap**, 0 by default: a solid wall). The two turns
at the seam are a line apart too, closing the tube's wall with a small notch between them.

- **Wings on the inside or the outside** (`wing_side`): outside, the same path mirrored outward
  from the tube's wall, which then stays the tube's inside (a thread inside).
- **Wings** (%): how far round they reach (for a shape with wings, the most they may; below). At 100 % each tip meets the facing tip a line apart
  (one seam: opposite the seam, a triple wall all round). **Past 100 %** a second wing grows the
  same way on the first wing's inner wall, at the seam (one fold deeper: two more walls), up to
  its own tips meeting at 200 % (five walls); and a third past 200 %, up to 300 %. At 150 % the
  triple wall is all round and the second wing goes half way.
- **Seams** (1–8): every seam gets its own pair of wings, spread evenly round the tube; 100 % is
  when each pair meets the next one's.

## How a wing starts

A wing starts where the tube's wall **parts at the seam**: at first the line only dips in (or
out) a little at a single point and carries straight on, so from outside the wall seems to open
a crease there. As it grows, its four turns go further round (the dent deepens, its sides
steepen), and the two sides move apart only as far as they must not to cross, so the gap
opens smoothly, never wider than it ends up: by the time the turns are a quarter round they're
as far apart as the finished turns at the seam (±g, below), and they stay there. Once the turns
are half circles the wings run back along the middle wall. A deeper wing (past 100 %) starts the
same way, on the inner wall at the seam.

"Two pieces a line apart" is exact: two turns facing each other (half circles a spacing `s`
across, their centres on a circle of radius ρ) just touch when their centres are `2s` apart,
`asin(s / ρ)` either side of the line between them. The tests check that no two pieces of a
layer come closer than a line (less the chord tolerance), as the wings grow and at full.

## Over the height

- **Straight start** (`wing_start`): the tube as it is today (the thread), printed by
  `vase_revolve` up to a whole number of turns, so its spiral ends exactly on the seam, where
  the wings carry straight on.
- **Or start with full wings** (`wing_bed`): no straight start; the wings, at their set %, from
  the first layer, which rises off the bed with each move extruded for the gap under it (as the
  spiral's first turn ramps; at a set flow, faster where the bead is thin). A brim goes round the
  outermost wall.
  The thread that screws onto the socket is then the wings' own. Inside wings screw on by their
  innermost wall, which sits at the socket's thread, so the tube's wall is two walls further out
  for every 100 % (three walls at 100 %, the socket inside the innermost). That wall only takes
  the thread rising along the wings, at a whole 100, 200 or 300 %. Outside wings screw on by the
  tube's wall, at the thread as usual, rising along the tube wall. Set otherwise, it says so.
- **Fade in** over `wing_in` mm, linear or smooth (an S-curve: starts and ends gently, for the
  overhang of each new bit of wing).
- **Full wings** for `wing_hold` mm, then **fade out** over `wing_out` mm (0: wings to the
  top), linear or smooth; above that, a plain tube to the top.
- **Diameter as the wings fade out:** the same, or the **same layer length**: the tube widens
  along the fade-out's curve (straight or an S-curve), from its own radius out to the plain
  circle as long as a layer at full wings, and the wings shrink exactly as much as keeps every
  layer that long; so the wall flares as smoothly as the curve, and the wings end where it's
  widest. (Driving the radius by the wings instead, the wall stayed narrow and then flared hard
  just where the wings ran out, next to their pointed tips: an odd outline.) The fade-in keeps
  the diameter.
- **Overhangs:** where the tube widens its wall leans out, and the Overhangs setting "layer
  height" applies as to any shade: layers come closer where it leans (each still extruded for a
  whole layer), by the lean's (1 − cos) scaled by strength and (lean / pivot)^steep. (Width
  compensation isn't used for the wings, and isn't kept up any more.) The fade, the widening and
  the lean are worked out on a fine grid of heights (every half layer) and the layers are laid
  along it, each its own rise, so the shape stays where it is and only the layers move; where it
  doesn't lean every layer is exactly a pitch (the thread's), and the top ends on the nearest whole
  layer.

## Fitting the shape

"Fit the shape" (the owner's idea, 15 Nov 2026) keeps the shape as set (opening, transition
height, sphere and so on) and gives it wings that follow from it: as much as keeps every layer as
long as the shade's widest one, so the layer time stays about the same where it narrows. None at
the widest; more where it's narrower (a sphere: near the throat and near the opening).

- **Wings** is the most they may take (up to 300 %); where even that isn't enough, the layer is
  shorter.
- The throat stays a plain thread; above it the wings fade in over "Fade in over" (an S-curve or
  straight), and follow the shape once the shape needs less.
- A lip (in the Lamp, a straight end) stays plain too: the wings stop where it starts, fading out
  over "Fade out over" up to there (0: at once).
- Each layer's length (its partial turns and all) is worked out every two layers at 12 amounts of
  wings (more of them near none, where the turns form), and the amount that gives the widest
  layer's length is read off between them. In the tests every layer that has its wings is
  within 3 % of the widest layer's length.
- In the Lamp it's a shade like the rest (its straight start screws on, its straight end takes
  more shades). Only the tube takes a thread along its wings (its wall is straight), and only
  with the fades (a curve changes along the way).

## Printed wide edge down

"Wide edge down" (in the Lamp, the Flip on the Plan and in the Prints view) prints the tube the
other way up: end first from the bed, the wings in reverse (a fade-out becomes a fade-in), its
straight start (the thread) last, at the top of the print. It's the same tube turned over (a
helix turned over keeps its hand, so the threads still match), so where the tube widens towards
its end that end goes on the bed and the widening becomes a lean inward. The first layer rises
off the bed as the spiral's first turn does; a brim goes round the first layer's outermost
wall. Each way round the top lands on its own nearest whole layer (where layers come closer,
they're counted from opposite ends), so the two can differ by a fraction of a layer.

## How it rises (`wing_rise`)

A pitch per layer, always; where it rises:

- **Whole layer:** steadily along the whole path by length, wings included: every bead is one
  layer above the bead under it.
- **Tube wall:** the tube's wall rises exactly a pitch per turn (a helix: a thread on it), the
  wings are level, and the turn back onto the wall at the seam rises what's left. With inside
  wings, a thread outside.
- **Wings:** the innermost wings' inner wall rises exactly a pitch per turn (the deepest wings',
  past 100 %), the tube's wall is level, and the other wing pieces rise what's left by length.
  With inside wings at 100 % the inner wall goes all the way round: a thread inside.

While the wings fade in (and out) the rise blends from "whole layer" into the chosen one, in
step with the wings' size, so the first wing layers never jump and every bead stays close to a
layer above the one below (2.3–2.5 mm at a 2.5 mm pitch, in the tests).

## In the Lamp

The Lamp's shade has the same shape and settings (the "Wings" group), with these differences:
- the straight start is the shade's own (`start_len`, or the joint length): its start ports are
  that long, not the whole tube's;
- the wings stop where a **straight end** starts (more shades screw on there), fading out over
  `wing_out` up to it (0: they stop at once, which prints fine: the tube's wall carries on below
  the end); full wings fill whatever is between the fade-in and the fade-out, so there's no
  "full wings for";
- the end ports are at the end's radius: wider, when the tube widens to keep the layer length;
- it has no closed end and never starts on the bed (it screws onto something);
- **printed wide edge down** (the Plan's and the Prints view's Flip) it prints end first, as below.

### Where it takes a thread

A face takes a thread only where it's a true helix: a wall that rises exactly a pitch a turn, at
one radius, all the way round. So a tube with wings offers ports (and the Plan offers add
buttons) only there:

| Where | Faces with a port | Why |
|---|---|---|
| Straight start | outside, inside ("Start") | the vase spiral |
| Straight end | outside, inside ("End") | the wings have stopped: plain turns a pitch apart |
| Full wings, rising along the **whole layer** | none | no wall rises a pitch a turn |
| Full wings, rising along the **tube wall** | the wall's free face ("Wings, outside" with the wings inside, "Wings, inside" with them outside), a line plus the fit off it | the tube wall is a helix |
| Full wings, rising along the **wings** | the deepest wings' far wall (inside: its inner face; outside: its outer face), only at a whole 100 %, 200 % or 300 % with full turns | that wall is a helix, with a one-line slot at each tip; at 150 % the deepest wings only go half way round |
| While the wings fade in or out | none | the rise is blending, and the tube may be widening |

The wings ports run along the full wings: from the end of the fade-in to the start of the fade-out
(or the straight end, or the top), radius and length as above, length 0 (no port) otherwise.

## How the recipe does it

General steps only (no new engine step). Per layer and wing level, the wing's size, the turns'
angle and positions are lists; every piece (the tube's wall, the middle and inner walls, the
turns) is an arc, about the axis or about a turn's centre, so one table of arcs (centre, radius,
start angle, sweep) describes a layer, seam by seam, level by level. Every layer has the same
number of points (as many as the chord tolerance needs on the widest wall); one formula draws
them all in print order from the table. Each piece's share of its layer's rise per point is
worked out from its arc's length or angle, so the heights are a running total along the
points. Printed with `toolpath` after the straight start's `vase_revolve`, through `join` (from
the bed: `bead` with a layer height that follows the gap). The steps that aren't used (any other
shape) run for one layer only.

## Limits and what's open

- **Speed:** about 26 ms per evaluation natively for the template as it ships (9,000 points);
  100–280 ms for a long, wide one (the owner's 533 mm tube with two seams outside, widening:
  55,000–124,000 points). The other shapes: 4–7 ms.
- **Size:** the engine's geometry budget (4 million numbers per evaluation) runs out only on very
  big ones (that 533 mm tube at 300 %); it says so. The points are made one short `series` per
  piece (a single `series` stops at 100,000 numbers).
- **Inside the tube** there must be room for every wall: the E27 tube holds 100 %, not 150 %
  (it says so: a wider socket, outside wings, or a narrower line).
- **The crease** at the very start of a wing is printed with the full bead where its sides are
  closer than a line: worth watching in the first print.
- **"Same layer length"** is worked out from the turns' lengths in closed form: within 4 % of the
  full wings' layer (the last fading layers come out a little short).
