# Patterns on the Lamp's and the Lampshade's sections

The owner's idea (22 Nov 2026): in the Plan, tap a section of a shade and put a pattern on it;
several sections picked, one pattern runs through all of them, fading in where it starts and out
where it ends. The first pattern: a sine wave in and out across the wall, matching up layer on
layer into long rails, or shifted half a wave each layer into a weave.

## What's set

In each shade (the Lampshade's own settings, the Lamp's per shade), group **Pattern**:

| Setting | What it does |
|---|---|
| Pattern on the straight start / fillet / shade / end fillet / straight end | Which sections carry it (the Lampshade: throat, fillet, shade, lip fillet, lip). Off as it ships. |
| Waves a turn | How many waves round each turn. |
| Waves stack into | **rails**: whole waves a turn, so every layer's wave sits on the one below: straight ribs up the wall. **woven**: half a wave more a turn, so each layer is shifted half a wave: a weave. |
| Depth | How far the wave pushes in and out (each way). |
| Fade in and out over | Along the surface (mm along the profile, not height): from nothing to the full depth where the run of picked sections starts, and back to nothing where it ends. |

The section switches are `@mark` settings: they're on their marks' bars in the Plan, not on the
shade's own bar (they're in the Settings tab as well).

## In the Plan

Tap a section (start, fillet, shade, end fillet, straight end): its bar shows its pattern switch
(and any other on/off setting it has, such as the straight end). Flip it; tap the next section
and flip that too. While a pattern is on, a **Pattern** mark beside each patterned section leads
to the pattern's settings.

## How it's made

One step on the shade's toolpath, after `vase_revolve` (before the brim and the closed end's
disc are joined on): `wave(plain_shade, amplitude: …, count: …)`.

- **The amplitude** is a field: for each point, its height on the profile (as the turns read it:
  turned over for a shade printed wide edge down or closed), then its distance along the profile
  from the start (`graph(full_z, pat_fs, …)`). Each section has its two ends along the surface.
  A picked section runs on into its neighbour when that's picked too, or empty (no fillet, no
  lip), so a run of picked sections fades only at its own two ends: smoothstep in over the fade
  length from the run's start, smoothstep out over it before the run's end. Two runs apart (the
  start and the shade, not the fillet) fade out and in again between them.
- **The count:** the waves a turn, plus a half for woven.
- **The `wave` step** (engine, the owner's leave to do general changes): an amplitude of 0 leaves
  a path exactly as it is (so nothing changes, G-code included, until a section is picked), and
  only the moves the wave reaches are split, each into as many pieces as give a wave 12 points by
  how far round it turns (the shade's spiral has no fixed count a turn; before, it split every
  move as if each were a whole turn: 288 pieces a move for 24 waves).
- The push is sideways, across the line in the horizontal plane (in and out of a round wall). On
  a wall that runs nearly flat it moves along the wall more than out of it.
- **Threads:** a pattern on the straight start (the throat) or the Lamp's straight end takes
  their thread away; the recipe says so (a warning), and the ports stay where they were. With
  wings, the straight start can't be patterned (its switch isn't offered).
- The shown wall (`wall`, the STEP export) is the plain profile: the pattern is in the toolpath.

Tests: `crates/core/src/patterns_tests.rs` (nothing changes until picked; the fade along the
shade; picked sections run on, two runs fade apart, the thread warning; rails stack and woven
shifts; the Lamp's shade, its marks and the straight end's warning), the `wave` unit test
`only_waved_moves_are_split`, and the smoke test (the switches from the marks' bars, the
Pattern mark).

Open: a test print (how deep a wave the layers hold, rails against woven); more patterns.
