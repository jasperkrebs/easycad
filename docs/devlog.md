# Dev log

Newest first. Each entry: what landed, numbers, anything left open (which also goes on the
roadmap's "Open" list). Keep it short; it's how the next session (human or agent) picks up.
Headings end with the branch that wrote them (`general-manager` or `recipe-developer`,
[working-together.md](working-together.md)).

Archives: [October 2026](devlog-2026-10.md), [November 1–14](devlog-2026-11.md).

## 2026-11-15: `when … and …`, a hand-built spiral's layer count (general-manager)

**Why:** the recipe developer's "Needs from the app" for the tube with wings: settings that hang
off a hidden one stay shown, and the app counted the wings (one long curve) as one layer; and a
Wings group of its own would have failed the smoke test.

**What landed:**
- `when A and B and …` on a `param` line: every condition must hold (each may have `not` and
  values), and every setting named must apply itself, as before. Parsed into `Condition.also`;
  the settings window, the engine's "no setting" warning, parts (names made the part's) and the
  STEP header's list of settings that apply all read it. Docs and the AI's guide updated.
- `toolpath(…, layers: n)`: how many layers a toolpath prints, for the count (0: one per curve).
- `join` doesn't count a print aid's layers (`aid`, a brim): the count is the object's.
- The smoke test checks the settings groups that show for the cone it opens with, so a shape's
  own group (hidden for a cone) can be added freely.

Tests: `a_when_can_join_conditions_with_and` (lang), `layer_counts_for_hand_built_spirals_and_brims`
(core), the `and` case in `settings.test.mjs`; the open-top golden test finds the last turn by
its turn number (with a brim the layer count is now one less). Golden G-code unchanged.

## 2026-11-15 (late night): a winged tube takes a thread only where it rises a pitch a turn (recipe-developer)

**Why:** the owner: shades must only be added where they can actually thread on, and that
depends on how the wings rise.

**What landed** (Lamp): two more ports on a tube with wings, "Wings, outside" and "Wings, inside",
along its full wings (between the fade-in and the fade-out). Rising along the tube wall: the
wall's free face; rising along the wings: the deepest wings' far wall, only at a whole 100 / 200 /
300 % with full turns; rising along the whole layer, or while the wings fade: no port (length 0),
so the Plan offers no add button and the settings no such mount. The straight start and end keep
theirs. Test `wings_take_a_thread_only_where_they_rise_a_pitch_a_turn`; the table is in
[tube-with-wings.md](tube-with-wings.md).

## 2026-11-15 (night): the tube with wings in the Lamp; layer height compensation only (recipe-developer)

**Why:** the owner wants to plan winged tubes in the Lamp, threaded onto and into other shades;
and uses only layer-height overhang compensation.

**What landed:** the Lamp's shade has the shape "tube with wings", with the Lampshade's wing
steps and a "Wings" group: its own straight start screws on (its start ports only that long),
the wings fade out up to a straight end (or stop there), and shades screw onto that end at its
radius, wider when it widened. Test `a_tube_with_wings_is_a_shade_too` (`lamp_tests.rs`): the
ports, plain circles in the straight end, full wings below, a cone on the widened end with no
collision. Width compensation is gone from the wings (both templates; "width" prints them at the
line width) and won't be kept up; the shades' "width" option stays for EasyGCode's golden tests.

**Open:** the Lampshade and the Lamp now share the wing steps as two copies too.

## 2026-11-15 (evening): overhang compensation where the winged tube widens (recipe-developer)

**Why:** with "same layer length" the tube widens as the wings fade out, and its wall leans out:
an overhang, which the owner wanted compensated as the shades' are.

**What landed** (Lampshade, tube with wings): the Overhangs settings apply: "layer height" brings
layers closer where the wall leans (each still a whole layer's bead). The fade, the widening and the lean are now worked out on a fine grid of heights,
and the layers are laid along it by their own rise, so the shape doesn't move and every layer
that doesn't lean is exactly a pitch; the top ends on the nearest whole layer. As it ships (no
widening) the path is unchanged to the last digit. Test `a_widening_tubes_overhang_is_compensated`.

## 2026-11-15 (later): wings that part at the seam; long tubes past 100 % (recipe-developer)

**Why:** the owner's renders: a wing started as a wide hole that the lines then grew back across
(its turns sat at the finished seam gap from the first layer), where it should start at the seam
and open smoothly; and their 533 mm tube with two seams outside failed past 100 % ("series:
count must be between 0 and 100000").

**What landed** (Lampshade): the turns now start together on the seam and move apart only as far
as the two sides need not to cross, reaching the finished seam gap by a quarter turn, so the
wall parts at a point and the gap opens (never wider than it ends up). The points are made one
short `series` per piece, in print order from the table of arcs, about 12 list steps a point
instead of 30: the template as it ships evaluates in 26 ms (was 50), and the owner's tube prints
at 150 % and 200 % (112,000–124,000 points). The wing settings have a group of their own
("Wings"), and with `when … and …` (general-manager) the settings that do nothing for a shape are
hidden: Wide edge down for the wings, the straight start and fade-in when starting from the bed,
the fade-out's settings unless there's a fade-out. `toolpath(layers:)` gives the app the real
layer count (213 for the owner's tube, not 17). Tests: the crease starts within 3° of the seam
and opens; the owner's tube past 100 %.

**Open:** the first print; "same layer length" is within 4 % (closed form).

## 2026-11-15: wings that start as a dent, outside, past 100 %, from the bed (recipe-developer)

**Why:** the owner's corrections and additions to the tube with wings: a small wing made four
tiny half circles (the whole shape scaled down), where it should only dip in at the seam and
turn back; and wings outside (a thread inside), a rise in the wings only (inside wings with a
thread inside), more than 100 %, and full wings from the bed.

**What landed** (Lampshade, [tube-with-wings.md](tube-with-wings.md)): a wing now starts as a
shallow dent whose turns grow from a few degrees to half circles before the wings run back;
`wing_side` (inside, outside); `wing_rise` "whole layer", "tube wall" (was "outer wall"), "wings"
(the innermost wings' inner wall a helix, the tube's wall level), blended in as the wings fade
in; `wings` up to 300 % (a second wing, then a third, grows on the inner wall at the seam);
`wing_bed` (full wings from the first layer, which rises off the bed through `bead`; a brim goes
round the outermost wall). The recipe is now one table of arcs per layer, seam and level, and
each piece's share of the rise comes from its arc, so it's as fast as before (about 60 ms as it
ships). Tests (`blocks_tests.rs`, `wings`): only forward moves while the dent is shallow; no two
pieces closer than a line as the wings grow, at 100 % inside and outside for 1–3 seams, and at
150–200 %; the tube wall's helix ("tube wall") and the inner wall's ("wings"); from the bed; the
errors and warning.

**Open:** as before (the first print; "Wide edge down" shows; the app's layer count).

## 2026-11-14 (late night): the Lampshade's tube with wings (recipe-developer)

**Why:** the owner's idea, from a sketch: a narrow tube's layers are too short to cool. Wings make
each layer longer without widening the tube ([tube-with-wings.md](tube-with-wings.md)).

**What landed:** a new Lampshade shape, "tube with wings": above a straight start (the thread,
printed as the tube is today), each layer turns back on itself at its seams onto a middle and an
inner wall, a line apart (or a line and a gap), the tips meeting a line apart at 100 %. Settings:
wings %, seams (1–8), gap, rise along the whole layer or the outer wall only (a thread outside),
straight start, fade in (linear or smooth), full wings for, fade out (0: none; linear or smooth),
and the diameter as they fade out (the same, or the same layer length: the tube widens). Small
wings start as a bump at the seam that grows. General steps only, printed with `toolpath` after
`vase_revolve` through `join`; no other shape's G-code changed (golden tests green). Tests in
`blocks_tests.rs` (`wings`): the start joins the straight part on the seam; one line, never
going down; the wings grow from a bump; no two pieces of a layer closer than a line (and the
tips meeting at a line) for 1–3 seams and with a gap; the outer wall on the thread's helix when it
rises alone; the widening keeps each layer within 3 % of full wings' length; the errors and the
warning. The new shape is also in `every_lampshade_shade_prints_both_ways_up`.

**Open:** the first print (watch the small bump's extra plastic at the seam); about 60 ms an
evaluation (the other shapes 4–7 ms); up to about 800 mm of wings (the geometry budget); "Wide
edge down" shows but does nothing for it; the app counts its wings as one layer. The settings
are in the Shade group for now (a Wings group of its own would change the smoke test's group
list, `scripts/smoke-test.cjs`). Not in the Lamp yet.

**Needs from the app** (for the general manager): a `when` that takes two conditions (`when
shape = "tube with wings" and wing_out`), so settings that hang off a hidden one stay hidden;
a way to give a hand-built spiral (`toolpath` of one long curve) its layer count, for the HUD.

## 2026-11-14 (night): a clear-out for working live (general-manager)

**What changed:** the roadmap no longer plans phases: it says what EasyCAD does today, the
decisions that stand, one **Open** list split by owner (app / recipes), and what's for later.
The dev log's November entries moved to `devlog-2026-11.md` (their open items are on the
roadmap now), and `CLAUDE.md`'s phase history became a short "where we are".

## 2026-11-14 (night): two agents, two branches (general-manager)

**What changed:** the owner split the work between two agents on long-lived branches:
`general-manager` (the app and its architecture: engine, language, app UI, G-code safety, build,
CI) and `recipe-developer` (templates, the owner's projects, tuning and test prints). The old
session branch is gone. Written down in `docs/working-together.md` (who owns which files, how a
recipe asks for a new step, merging `origin/main` before every task and every pull request,
conflicts, dev log entries ending with the branch), `CLAUDE.md` (collaboration workflow, read
order) and the README.
