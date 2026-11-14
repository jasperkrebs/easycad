# Dev log

Newest first. Each entry: what landed, numbers, anything left open (which also goes on the
roadmap's "Open" list). Keep it short; it's how the next session (human or agent) picks up.
Headings end with the branch that wrote them (`general-manager` or `recipe-developer`,
[working-together.md](working-together.md)).

Archives: [October 2026](devlog-2026-10.md), [November 1–14](devlog-2026-11.md).

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
