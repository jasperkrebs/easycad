# Dev log

Newest first. Each entry: what landed, numbers, anything left open (which also goes on the
roadmap's "Open" list). Keep it short; it's how the next session (human or agent) picks up.
Headings end with the branch that wrote them (`general-manager` or `recipe-developer`,
[working-together.md](working-together.md)).

Archives: [October 2026](devlog-2026-10.md), [November 1–14](devlog-2026-11.md).

## 2026-11-17: works offline again, every file (general-manager)

**Why:** the owner asked whether the app works without WiFi. It does (the engine runs in the
browser; only the Chat tab needs the internet), but the offline cache's list in `web/sw.js` was
written by hand and had fallen behind: the Plan's code (`js/parts.js`), the sketcher, and the
Lamp and Thread pendant templates were missing, so offline they worked only if used online first.

**What landed:** the missing files are on the list, and a test (`settings.test.mjs`) fails when a
shipped file (code, template, icon, vendored library) isn't, naming it. Checked in Chromium:
opened once online, then offline: reloads, the engine starts, the Lamp and Thread pendant open.
A new template or script must be added to `web/sw.js` (the test says which).

## 2026-11-16 (late): the Thread pendant, from the Thread Lampshade Sketcher (recipe-developer)

**Why:** a friend of the owner's built a Claude artifact, the Thread Lampshade Sketcher: wings
that run in from the wall and close into rings that thread onto the socket. The owner: make its
pendant here, printable, reading its settings files natively, E27 (E14 later); the general
manager's part allowed for this one.

- **`thread_shade`** (`crates/core/src/pendant.rs`): the sketcher's generator ported to Rust,
  pendant with the wings on the socket; the same points as the sketcher's own for its four files
  (to 0.00002 mm, the browser's floats; a test keeps samples of two). A project's own step, against
  "general steps" on purpose: it keeps the files and the shade the same.
- **Printable:** the first layer on the bed (`bead`), overhang compensation by layer height below
  the thread (never in it: the grip stays a true helix, tested), line width and flow as settings,
  the sketcher's checks as warnings plus one: wings stepping more than half a line a layer (its
  65–70° files do, at 6.9 mm a layer).
- **The template** `thread-pendant.easycad` (the sketcher's settings by name), in the template list.
- **The files:** Open or Load settings takes a sketcher `.json` (`web/js/sketcher.js`, tested
  with its files in `fixtures/sketcher/`, `node --test scripts/sketcher.test.mjs`, in CI); "Save
  for the sketcher" writes one back.
- Found by the sweep: a custom pitch rounded the first point a hair under the bed (fixed, tested).

Open: a test print; the E14 barrel; a pace for the wing rings; floor lamp, linear pendant and
screw part not made ([thread-pendant.md](thread-pendant.md)).

## 2026-11-16 (night): the wings' outline follows the % smoothly (recipe-developer)

**Why:** the owner's sphere (eight seams, a curve down to none): "always that pointy bit that
doesn't follow the curve", at every setting. It was the recipe: while a wing formed, the opening at
the seam rushed to its full width, then the tips waited there until the turns were half circles
(in that file: 5.5° for four layers), then ran on: a stall and a jog in the outline.

- Now the tips reach round in step with the % from the first layer; the opening widens with them
  up to the seam's angle, the turns form under them over about twice that (slowly at first, so the
  opening never needs more than the reach). The middle wall sits up to a tenth of a line over the
  wall for a few layers while a turn forms (the growth test allows that, full wings stay strict).
- No button needed: it's how every wing forms now. The tips' pace counts each level's forming
  over its own radius (a deeper level's was counted too fast).
- Test: the owner's sphere, every growing layer's tips within 20 % of the % (they stalled at 0).
  The fit-the-shape test skips layers under 3 % of wings and a bell's last two (lengths every two
  layers are too coarse where it flares flat).

## 2026-11-16 (evening): wings tried on every shape: a pace, room past 100 %, a choice of start (recipe-developer)

**Why:** the owner: the fade-in couldn't be found, wings past 100 % didn't work, "a few things
don't work". A scratch survey (every shape, in and out, 50–300 %, seams, gap, curve, fit, wide edge
down, lip) checked each print for beads with nothing under them and lines crossing in a layer.

- **Fade-in hidden on every shape but the tube:** its settings named `wing_bed` (tube only), and a
  setting shows only when every setting it names applies. Fixed; the bed start now applies only to
  the tube in the recipe too.
- **Wings start: fading in / full** (new): full puts the wings at their set % on the first layer.
- **Tips grow at most ½ line a layer** (`wing_grow`, new; 0: no limit): grown faster, wings on a
  cone or sphere hung in the air at their tips (up to 26 mm gaps) and their forming sides spread
  ~5 mm a layer. Now every bead in the survey has one under it, apart from the bell's and dome's
  own flat tops. Warns when the wings can't reach their peak at that pace.
- **Past 100 % inside:** levels by the room at each layer's radius (was an error on any E27 shade).
- **Fixed:** a level not started yet with its radius below zero drew a loop and a whole circle at
  the seam (cone 300 %); dividing by zero with no pace limit; a curve wide edge down starting at 0
  instead of its value; outside wings on a widening cone crowding the next layer (each point now
  follows the shade's radius at its own height); the dents warning on wings the room or pace capped.
- Tests: the pace (no bead hanging, inside and out; hanging without it), the full start, deeper
  wings on a wide cone (no crowding), 200 % inside on E27 (a warning, 100 % printed). The wing
  tests of other features run without the pace.

Open: outside 200 % with "same layer length" widens the tube from 23 to 144 mm over the fade-out
(a 60° flare; flipped, unprintable): maybe cap it. Tips on a narrowing top (sphere, in-out) still
overhang a little. Default pace (½ line) to check in a print.

## 2026-11-16 (latest): wings end on a whole layer, and grow without a kink (recipe-developer)

**Why:** the owner's cone (wings outside, four seams, a curve from 100 % at the throat down to
20 %, wide edge down) had one odd loop next to the throat, and a kink in the outline where the
wings went from growing round to growing along.

- **The loop:** each layer read its wings at its middle, which for the last one was half in
  the plain throat: 57 %, between the full wings and none. Now each layer reads them inside the
  wings' own stretch, carried on along the curve's slope past its last grid height (never up),
  so the last layer by the throat has 99.6 % and a bell's last layers still fade to ~2 %.
- **The kink:** a tip grew by its turn's angle up to a point, then by the tip's angle: two
  slopes meeting. The two now blend over half a turn either side (a quadratic), so the tip's
  angle has no corner as the % changes.
- A test with the owner's settings: the last winged layer ≥ 95 %, every winged layer 20–100 %,
  no jump over 0.12 between layers.

Open: nothing new.

## 2026-11-16 (later): Wings keep "wide edge down" in sight (recipe-developer)

**Why:** switching Wings on hid "Wide edge down": it shows while the shade isn't closed, and
Wings hid "Closed end" (a setting only shows while the settings its `when` names show too).

**What landed** (Lampshade and Lamp): the other way round: "Closed end" always shows and "Wings"
shows while the shade isn't closed (a closed end prints disc down, wings or not; closed, the shade
has no wings). Checked in the app: with Wings on, Wide edge down shows; closing the shade hides
Wings and Wide edge down.

## 2026-11-15 (evening): overhangs keep their plastic (general-manager)

**Correction:** the roadmap's overhang note suggested extruding less on squeezed turns and a floor
on the rise. The owner doesn't want either: every turn keeps the same plastic for even layer
lines, and the compensation is only how close the turns sit. The note now says so, with tests 5
and 6 (test 6: 80 %, steep 0.8, pivot 76, opening 160 mm).

## 2026-11-16: wings are a setting of every shape (recipe-developer)

**Why:** the owner wanted wings and shape set independently: any shape, wings on or off like the
brim, and the wings following the fades, a curve of their own, or the shape.

**What landed** (Lampshade and Lamp, [tube-with-wings.md](tube-with-wings.md)): the shapes are the
plain ones again (no "tube with wings", "cone with wings"…; old projects pick their shape again
and switch Wings on). "Wings" at the end of the Shade group switches on a "Wings" group: "Wings
follow" fades (the %, faded in and out; the tube can widen to keep the layer length), a curve
(`wing_curve`, rows of where along the shade 0..1 and %, a smooth curve through them; the fades
are not used), or fit the shape (as long as the widest layer). Inside/outside, seams, gap, rise,
start from the bed (the tube) as before. Tests `every_shape_can_have_wings` (9 shapes × 3 modes)
and `the_wings_can_follow_a_curve_along_the_shade`; the earlier tests on the new settings.

## 2026-11-15 (night, last): the widening follows the fade-out's curve; points per arc (recipe-developer)

**Why:** the owner's outlines: where the wings ran out, the wall flared hard right beside their
pointed tips. With the wings driving the radius, the wall barely widened while the wings shrank
slowly and then jumped out in the last stretch, as the turns flattened into dents.

**What landed** (Lampshade and Lamp): with "same layer length" the tube's radius now follows the
fade-out's curve from its own radius to the plain circle as long as a full-wings layer, and the
wings are as much as keeps each layer that long (the same table as the shapes with wings). And
every arc gets as many points as the chord tolerance needs at its own radius and sweep (one when
there's nothing to draw): the template as it ships drops from 9,100 points to 5,300, the owner's
259 % tube from 115,000 to 52,000. Test `fading_out_the_tube_can_widen_to_keep_each_layer_as_long`
now checks the wall half way out half way up a straight fade-out.

## 2026-11-15 (night, later): a widening tube no longer narrows where a level of wings ends (recipe-developer)

**Why:** the owner's tube (from the bed, 259 % outside on 6 seams, widening as the wings fade out)
narrowed in two places on the way up. Made before today's exact-length fix, it still guessed the
layer lengths, worst where a level of wings runs out; with the fix it was just over the engine's
geometry budget (115,000 points through ~20 list steps each).

**What landed:** the points are built with fewer list steps (each coordinate in one `flatten`),
so it fits; its wall widens all the way (16.5 to 87 mm). Test
`a_tube_widening_through_three_levels_of_wings_never_narrows`. With outside wings the outermost
level's walls end where the wings drop below 200 %: the outline steps in there by about two walls
(that's the wings, not the wall).

## 2026-11-15 (night): from the bed, the wings screw onto the socket (recipe-developer)

**Why:** the owner: with full wings from the bed (no straight start), the socket screws into the
wings themselves, so the innermost wall must fit the socket.

**What landed** (Lampshade): from the bed, inside wings put their innermost wall at the socket's
thread (the tube's wall two walls further out per 100 %); outside wings keep the tube's wall
there. A warning when the rise can't make that wall a thread (inside: rise along the wings at a
whole 100/200/300 %; outside: rise along the tube wall). Test
`from_the_bed_the_wings_screw_onto_the_socket` (both sides: the wall at the thread's radius, a
helix a pitch a turn from the first layers; the warnings).

## 2026-11-15 (evening): cone, bell, dome, sphere and flat sphere with wings (recipe-developer)

**Why:** the owner wanted the shades with wings too, but with the shape set first and the wings
following from it, rather than the % driving the shape.

**What landed** (Lampshade and Lamp, [tube-with-wings.md](tube-with-wings.md)): five more shapes,
"X with wings", each the shape X with wings that keep every layer as long as its widest
(at most the Wings %, faded in from the plain throat, stopping at a lip or straight end). The
same table that now works out the tube's widening works out the wings: per height, at 12 amounts
of wings, the amount that gives the widest layer's length. Tests
`a_shape_with_wings_keeps_its_layers_as_long_as_its_widest` (all five: within 3 % once faded in,
never over; the throat plain; wide edge down) and `a_shape_with_wings_is_a_shade_too` (a sphere
with wings in the Lamp, a straight end that takes a cone, no wing ports).

**Open:** the Plan's add buttons give new shades sizes by their shape's name (`web/js/parts.js`):
"cone with wings" etc. come with the component's defaults.

## 2026-11-15 (later): "same layer length" works out each layer's real length (recipe-developer)

**Why:** the owner's flared tube (8 seams outside, a 10 mm gap) bulged and necked where its wings
ran out: the tube's radius grew, shrank, grew again. The length of a layer while its turns
form was guessed (the full turns' length scaled down), far off with a gap; and with no fade-in
the first height had no wings (0 ÷ 0), its radius leaking into the next.

**What landed** (Lamp and Lampshade): a layer's exact length (partial turns and all) is worked
out every two layers up at 12 radii and the radius that keeps the longest layer's length is read
off between them, no Newton steps; "fade in over 0" is full wings at once; the widening starts
exactly where the fade-out does; the wings start on the straight start's end even when they
start wider (no hop). Test `a_flaring_winged_tube_widens_smoothly_to_the_end_of_its_wings` (the
owner's settings: the wall only widens, no bunched layers, one line).

## 2026-11-15 (later): STEP is the surface, not a thickened wall (general-manager)

**Why:** the owner works on exported shapes as surfaces; a wall thickened to the line width was
the wrong thing to hand over. STEP doesn't need a solid: a surface goes in as an open shell.

**What landed:** the Lampshade and every Lamp shade export `wall` itself (the surface through
the bead's middle) as STEP: one open shell of exact cones, cylinders and planes, no solid, the
sizes as designed (the default lampshade 150 mm across, was 156 thickened). `thicken` stays as a
step for a recipe that wants a solid. Checked with Open CASCADE 8: the default lampshade, a
closed bell in-out, the tube with wings and a Lamp shade each read as one valid shell. Tests and
the smoke test expect the surface model; docs and the AI's guide updated.

**For the recipe developer:** the tube with wings' `wall` is the plain tube, so its STEP has no
wings (on the roadmap).

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

## 2026-11-15: a tube with wings prints wide edge down too (recipe-developer)

**Why:** the owner flipped a winged tube in the Plan (it hangs down from the socket, widening at
its end) and it still printed start first.

**What landed** (Lamp and Lampshade): "wide edge down" works for the tube with wings: the whole
tube end first from the bed (the schedule of the wings read from the other end, the first layer
rising off the bed through `bead`, a brim round its outermost wall), its straight start last;
shown in the same place in the lamp, its ports where they were. Tests
`a_tube_with_wings_can_print_wide_edge_down` (the owner's tube: on the bed at its widened end,
plain thread at the top, the same radii at every height turned over, the same span in the lamp)
and `the_tube_can_print_wide_edge_down` (Lampshade).

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
