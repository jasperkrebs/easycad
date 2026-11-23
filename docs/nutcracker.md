# Nutcracker

*21 Nov 2026, the owner's idea: from a Lamp project (an 80 mm threaded tube screwed into a closed
cap), its own template, the tube flattened above the cap into a key you turn.*

Two prints from one set of settings, each with its own G-code file:

- **The cap:** a flat spiral bottom (rings linked from the centre out, the salt shaker's), a fillet,
  then a straight wall in vase mode at the thread's pitch: every turn lays one turn of thread.
- **The key:** printed its pressing end down: the same bottom and fillet, the threaded tube
  ("Threaded length", as long as the cap is deep and a bit more), then **the flattening**: over
  its length each layer goes from the tube's circle to the handle's stadium (two straights and two
  half circles), along an S-curve or straight, then **the handle**, flat for its length.

The cap's wall sits one line width outside the key's (plus the fit); both spirals turn the same
way, so the key screws in closed end first, as it prints. Put the nut on the cap's floor and screw
the key down onto it.

## The bottoms: a cross

The bottoms take the force, so each is tied to its wall: as soon as the wall has climbed a layer
above the bottom (part way up the fillet), the line runs in to the middle, turns round near it and
runs back out, as many times as the part's spokes are set (`Cap: spokes`, `Key: spokes`; 4: a
cross), evenly round, each spoke two legs a line apart, flat on the bottom's spiral (crossing its
rings), the bit of wall between the legs left out. More (`Cap/Key: how many, on top of each
other`, up to 5) go a layer up each, flat on the one below, their spokes over its spokes: all start
from the seam's angle (+Y). The engine step `spokes` does it (any vase with a bottom can use it;
its `angle` puts the first spoke where the path next passes that angle, so spokes at several
heights stack; `layers` and `layer` stack them in one step).

**How long the spokes are** (23 Nov; tested): a spoke is two beads side by side, a band two lines
wide (`w` the line), its end rounded with one line's radius round the turn's middle, `c` from the
axis. Two neighbouring bands at the angle 360°/n between them first touch at their ends, where
their middles are 2w apart: 2·c·sin(180°/n) = 2w, so **c = w / sin(180°/n)**: 2 spokes meet tip to
tip at the axis (c = w), 3 like the Mercedes star (1.15w), 4 at 1.41w (was 1.25w, tips overlapping a
little), 6 at 2w, 8 at 2.6w, 12 at 3.9w: the more there are, the shorter. One spoke runs right
across: it turns round a bead off the opposite wall, c = −(R − 1.5w), R the wall's radius there.

**Or a spiral** (`Cap/Key: tied to the wall by`: spiral): a second bottom in one line, on top of
the first. Where the wall reaches two layers, the line turns back and runs in, against the wall's
way (the other way round from the bottom's spiral), round on rings a line apart (a hair less, so
they come out even to half a line from the middle), stepping in two rings at a time in one short
window just on from where it went in; in the middle it turns round (half a circle) and runs back
out on the rings between, the wall's way, stepping out in the same window, a ring from the way
in's steps; its last step climbs onto the wall and carries straight on up it (the wall between,
a few lines long, left out at that height). Rings, not a steady spiral, so the outermost one
runs a line from the wall all round (a steady spiral leaves a crescent). It turns back once, where
it goes in, and leaves a small gap there (about a line square, between the way in and the wall).
More, a layer up each, the same way (`spiral_tie`, `count`, `layer`).

## Overhangs and the thread

The fillets (and the key's flattening) lean, so their layers come closer: overhang compensation
by layer height, as the Lampshade's (Overhangs: strength, steep curve, pivot), each layer still
extruded for a whole one. The straight walls are the threads: every turn exactly the pitch. They're
sliced on their own (the fillet in compensated steps fitted to its height, then the straight wall
at the pitch): sliced as one, the steps were fitted to the whole height and the threads came out
1 % off, the cap's and the key's differently (2.474 and 2.478 mm a turn: half a millimetre apart
over the cap).

**The flattening, point by point** (23 Nov): it leans differently all round, its sides moving in
(43 → 3 mm, up to about 37° at the S-curve's middle), its ends moving out (43 → 64.5 mm, about
20°), the corners hardly. Paced as a whole, every point got the steepest point's compensation:
the sides were no better off and the rest was squashed. Now each point down the wall is paced by
its own lean (`contour(…, across: true)`), all of them taking the same number of layers from the
tube to the handle, so the layers come closer only where the wall overhangs, flat where the
flattening starts and ends and leaning a little between (non-planar). Its own strength
(`Flattening: strength`, 86 % like the rest): with the shared steep curve (0.85, pivot 73°) a
37° lean asks for only about 10 % closer layers (2.24 mm in the middle of the sides against
2.32 at the ends); if the sides still sag, raise it, or take the steep curve to 0 (at 100 %:
about 2.0 mm there), without squashing the ends or the threads.

## The handle

- **Width:** 0 makes it as long round as the tube (its bead's middle), so every layer takes as
  long as the tube's and the line isn't stretched; or set the width (outside).
- **Thickness** (outside): 0 (as it ships) is solid: its two walls close in to a line apart, centre
  to centre, so they touch whatever the line width (it was 12 mm, two 6 mm lines: with a 5 mm
  line its walls stayed 2 mm apart, 23 Nov). Set, more than two lines leaves it hollow.
- The flattening leans its long sides in and its ends out: layers come closer there (Overhangs),
  and past 60° it says to make it longer.

## How it's built

General steps (`web/templates/nutcracker.easycad`): sections (`circle`, `rectangle` with
half-round ends, `tween` between them) lofted into surfaces piece by piece, `contour` into layers
(the compensated layer height a field of z), `spiralize` into one vase line, joined to the
bottom's `rings` / `link_rings` spiral, then `spokes` for the cross (new, 22 Nov: in
`toolpath_ops.rs`). Tests:
`crates/core/src/nutcracker_tests.rs`.

## Open

- A test print: the fit, and how much the handle takes before the thread slips.
- Maybe a grip (holes or ribs) on the handle, or a textured cap.
