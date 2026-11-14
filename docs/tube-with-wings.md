# Tube with wings (Lampshade)

*The owner's idea (14 Nov 2026), from a sketch; first built in the Lampshade template as the
shape "tube with wings". Later it may become a shade in the Lamp too, so it can thread into
another shade.*

## Why

A narrow tube prints short layers: each one is done before the one below has cooled, and the
wall slumps. Wings make every layer longer **without making the tube any wider**: at the seam
the line turns back on itself inside the tube, so each layer has more to print (more time to
cool) while the outside stays the same.

## The path, one layer

Starting on the outer wall and going round (counter-clockwise from above, like the spiral):

1. the outer wall, round to the seam;
2. a half-circle turn **inward** onto a middle wall, then **back** against the print direction
   (the first wing);
3. another turn inward onto an inner wall, **forward** through the seam and past it as far again;
4. a turn **outward** onto the middle wall, back to the seam (the second wing);
5. a turn out onto the outer wall, and on round: the next layer.

So the inner wall is one arc through the seam and the middle wall two arcs, one on either side
of it. The walls sit a line apart (plus the **gap**, 0 by default: a solid wall). The two turns
at the seam are a line apart too, so they close the outer wall with a small notch between them.

`wings` (%) is how far round they reach: at 100 % each tip meets the facing tip a line apart
(with one seam they meet opposite the seam, and the wall is three lines thick all round); at
0 % it's a plain tube. With more **seams** (2, 3, …) every seam gets its own pair of wings,
spread evenly round the tube, and 100 % is when each pair meets the next one's.

Small wings are the whole shape scaled down about the seam: they start as a tiny bump on the
outer wall that grows, layer by layer, into the turns and then the wings, so nothing appears
in mid-air.

"Two pieces a line apart" is exact: two turns facing each other (half circles a spacing `s`
across, their centres on a circle of radius ρ) just touch when their centres are `2s` apart,
`asin(s / ρ)` either side of the line between them. The tests check that no two pieces of a
layer come closer than a line (less the chord tolerance).

## Over the height

- **Straight start** (`wing_start`): the tube as it is today (the thread), printed by
  `vase_revolve` up to a whole number of turns, so its spiral ends exactly on the seam, where
  the wings carry straight on.
- **Fade in** over `wing_in` mm, linear or smooth (an S-curve: starts and ends gently, for the
  overhang of each new bit of wing).
- **Full wings** for `wing_hold` mm, then **fade out** over `wing_out` mm (0: wings to the
  top), linear or smooth; above that, a plain tube to the top.
- **Diameter as the wings fade out:** the same, or the **same layer length**: the tube widens
  as the wings go, so each layer stays as long as at full wings (with no wings left it's a
  plain tube about three times as wide, for one seam at 100 %). The fade-in keeps the diameter.

## How it rises

- **Whole layer:** steadily along the whole path, wings included: every bead is one layer above
  the bead under it. The outer wall's rise per turn is then only part of a layer (the wings take
  the rest), with a small step at the seam.
- **Outer wall:** the outer wall rises exactly a pitch per turn (the thread's helix), and the
  wings are level; the last turn back onto the outer wall rises across the seam. For a tube that
  has to thread on its outside.

## How the recipe does it

General steps only (no new engine step): per layer, the reach, the radius and the turns' sizes
are lists; every layer has the same number of points, in eight pieces (outer wall, turn, middle,
turn, inner, turn, middle, turn). Each kind of piece is computed for all its points at once,
then put in print order with one `item` lookup, and printed with `toolpath` after the straight
start's `vase_revolve`, through `join`. The point counts follow the chord tolerance, as the
spiral's do. The steps that aren't used (any other shape) run for one layer only.

## Limits and what's open

- **Speed:** about 60 ms per evaluation natively for the template as it ships (8,000 points),
  more with more seams or a wider tube (the other shapes: 4–7 ms).
- **Length:** the engine's geometry budget (4 million numbers per evaluation) runs out at about
  800 mm of wings on the E27 tube with one seam; it says so.
- **The small bump** overlaps its own beads (three walls closer than a line) until it's full
  size: a little extra plastic at the seam in the first layers of the fade-in. Worth watching in
  the first print.
- **Wide edge down** still shows for this shape but does nothing (it always prints throat down,
  open at the top).
- The app's layer count says 17 (16 turns of the straight start + 1 for the wings); the G-code
  is right.
