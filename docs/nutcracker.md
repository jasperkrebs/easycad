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
runs back out, `Spokes on the bottom` times (4: a cross), a quarter turn apart, each spoke two legs
a line apart, flat on the bottom's spiral (crossing its rings), the bit of wall between the legs
left out. The engine step `spokes` does it (any vase with a bottom can use it).

## Overhangs and the thread

The fillets (and the key's flattening) lean, so their layers come closer: overhang compensation
by layer height, as the Lampshade's (Overhangs: strength, steep curve, pivot), each layer still
extruded for a whole one. The straight walls are the threads: every turn exactly the pitch. They're
sliced on their own (the fillet in compensated steps fitted to its height, then the straight wall
at the pitch): sliced as one, the steps were fitted to the whole height and the threads came out
1 % off, the cap's and the key's differently (2.474 and 2.478 mm a turn: half a millimetre apart
over the cap).

## The handle

- **Width:** 0 makes it as long round as the tube (its bead's middle), so every layer takes as
  long as the tube's and the line isn't stretched; or set the width (outside).
- **Thickness** (outside): at two lines its two walls meet into one solid blade (the default, 12
  mm with a 6 mm line); thicker leaves it hollow.
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
