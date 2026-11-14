# Tube with wings (Lampshade)

*The owner's idea (14 Nov 2026), from a sketch; built in the Lampshade template as the shape
"tube with wings". Later it may become a shade in the Lamp too, so it can thread into another
shade.*

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
- **Wings** (%): how far round they reach. At 100 % each tip meets the facing tip a line apart
  (one seam: opposite the seam, a triple wall all round). **Past 100 %** a second wing grows the
  same way on the first wing's inner wall, at the seam (one fold deeper: two more walls), up to
  its own tips meeting at 200 % (five walls); and a third past 200 %, up to 300 %. At 150 % the
  triple wall is all round and the second wing goes half way.
- **Seams** (1–8): every seam gets its own pair of wings, spread evenly round the tube; 100 % is
  when each pair meets the next one's.

## How a wing starts

A wing starts as a **shallow dent**: the line follows the tube's wall, turns in a few degrees at
the seam and straight back out, and carries on. As it grows, its four turns go further round
(the dent deepens, its sides steepen to straight in and out at 90°), until they're half
circles; only then do the wings run back along the middle wall. The turns start where the half
circles will be, far enough apart that the two sides of the dent never come closer than a line.
A deeper wing (past 100 %) starts the same way, as a dent in the inner wall at the seam.

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
- **Fade in** over `wing_in` mm, linear or smooth (an S-curve: starts and ends gently, for the
  overhang of each new bit of wing).
- **Full wings** for `wing_hold` mm, then **fade out** over `wing_out` mm (0: wings to the
  top), linear or smooth; above that, a plain tube to the top.
- **Diameter as the wings fade out:** the same, or the **same layer length**: the tube widens
  as the wings go, so each layer stays as long as at full wings (with no wings left it's a
  plain tube about three times as wide, for one seam at 100 %). The fade-in keeps the diameter.

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

- **Speed:** about 60 ms per evaluation natively for the template as it ships (9,000 points),
  150–220 ms with 8 seams, 300 % or a wide tube (the other shapes: 4–7 ms).
- **Length and size:** the engine's geometry budget (4 million numbers per evaluation) runs out
  on very big ones (8 seams at 300 % on a 100 mm thread); it says so.
- **Inside the tube** there must be room for every wall: the E27 tube holds 100 %, not 150 %
  (it says so: a wider socket, outside wings, or a narrower line).
- **The dent** is printed with the full bead where its sides are closer than a line (only in the
  first few layers of each wing, where it is shallow): worth watching in the first print.
- **Wide edge down** still shows for this shape but does nothing (it always prints throat down,
  open at the top).
- The app's layer count says 17 (16 turns of the straight start + 1 for the wings); the G-code
  is right.
