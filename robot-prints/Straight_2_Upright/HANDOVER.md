# Straight 2 on the KUKA, nozzle upright: handover

Made with EasyCAD on 20 November 2026 for the KUKA KR 50 R2500 (KRC5, KSS 8.7.9) pellet cell of
the SPHR handover (`SPHR_Full_Print_Handover-2/`): the same robot, tool, bed, frames, load,
extruder commands, start, purge and finish. The part is the owner's `straight-2` Thread pendant
(a straight tube, 145 mm across the bead's middle, 248 mm tall, three wing entries, an E27 grip),
printed with the **nozzle always upright** (no tilt), opening down on the bed.

## Start here

- For the whole print, use `Full/STRAIGHT2.zip`. For a short test, use `Test_40mm/STRAIGHT2.zip`
  (the first 40 mm, then the normal finish). Each ZIP holds `STRAIGHT2.SRC` and `STRAIGHT2.DAT`.
- Open `Full/Robot_Simulation.html` (or the test's) in a browser to watch it: Play, scrub,
  **Frame: Whole robot** for the pedestal and the arm, **Print area** for the part.
- `straight-2-robot.easycad` is the project: open it in EasyCAD (Settings, Open) to change it.
  Its `printer` line picks the KUKA; the Print tab's **Robot program** button writes the ZIP
  (its **Stop at this height** makes a test). The app names the program `EASYCAD`; this package's
  is `STRAIGHT2`.

## What it prints

| | Whole print | Test |
|---|---|---|
| Height | 248 mm (165 layers by turns) | 40 mm (27 turns) |
| Line | 141.5 m of bead, 704 cm³ | |
| Nominal time (moves only) | 1 h 38 min | 8.5 min |
| Spline segments (SPL) | 21,906, in 22 SPLINE blocks of at most 1000 | 1,268, in 2 blocks |
| SRC size | 1.97 MB (25,230 lines) | 124 kB |

- **Bead:** the outer wall and wings 3.5 mm wide × 1.5 mm layers; the thread in the grip 5 mm ×
  2.5 mm (the E27 pitch), growing from one to the other before the grip.
- **Speed and extruder:** flow 150 mm³/s, but no move faster than 25 mm/s, so the thin wall runs
  at 25 mm/s (about 119 mm³/s, screw command about 3,580) and the thread at about 13 mm/s
  (screw 4,493, the peak). The handover's globe peaked at 21.3 mm/s and command 6,390.
  Screw commands are 30 units per mm³/s, worked out from the handover's own wall (300 per mm/s with
  a 4.5 × 2.5 mm bead): **not measured; calibrate with a purge first.**
- **Nozzle:** upright the whole way (A −90, B 0, C 0 in the base frame). No tool roll and no tilt
  relief were needed.

## Program, as the handover's

Line for line the handover's 30-layer program where they do the same thing:

1. Frames from the DAT (TR_TOOL X −508, Y 3.5, Z 139.34, A −180, B −90, C 0; TR_BASE Y 50,
   Z −0.3), the load (LOAD_DATA[6] with 18 kg at the estimated centre of mass), orientation
   limits, `$IPO_MODE = #BASE`, `$ORI_TYPE = #VAR`.
2. Heaters T0 220, T1 190, T2 70 °C, part fan off, firecap fan 5, **HALT** (heat up).
3. To the purge point (world 1088.30, 134.86, −590) upright, **HALT**, `WAIT SEC 0`, screw 12000
   for 20 s, then the first print command; to 5 mm above the first point at 100 mm/s and down
   onto it at 20 mm/s.
4. The print: spline restrictions on (old ones saved), `SPLINE WITH $ACC={CP 0.050, ORI1 150.0,
   ORI2 150.0}`, one `SPL` per point with its own `$VEL.CP`, extruder changes and the part fan
   (255, from the second layer) as `TRIGGER WHEN PATH=0 ONSTART` subroutines. Blocks joined with
   `ENDSPLINE C_SPL`.
5. Finish: `WAIT SEC 0`, extruder stopped, screw 0, heaters 0, lift 50 mm, upright, fans off,
   spline settings restored.

Print centre world X 1088.304936, Y −115.143158 on the measured bed (world Z −659), as the
handover's.

## Simulation and checks

Replayed on the handover's own robot model (its `kinematics.py`, `proxy_collision.py`, CAD
proxies and fan housings) with EasyCAD's `scripts/robot-check.py`; each folder's
`collision_report.json` is the full report.

| | Whole print | Test |
|---|---|---|
| Poses checked (targets + 3 between each two) | 87,641 | 5,089 |
| Joint limit violations | 0 | 0 |
| Least joint margin | A3 36.0°, A5 36.8° | A5 37.4° |
| Tool against arm, arm against itself, pedestal | none | none |
| Tool to bed (all but the nozzle; target 10 mm) | 23.3 mm (heater collar), fans 38.3 mm | same |
| Tool to the printed part (fans' target 5 mm) | heater collar 18.2 mm; every other part at least 25 mm | heater collar 22.2 mm |

EasyCAD also checks every pose on the same kinematics while writing the program and refuses one
that's out of reach or within 3° of a joint limit.

**Not covered (as in the handover):** the controller's own spline and blending (the replay is
straight between targets at the programmed speed), acceleration, cables and hoses, cell fixtures,
the move from wherever the robot starts (`PTP $AXIS_ACT`), sag of the print. **Needs the
controller:** that it compiles and runs the program (the `ENDSPLINE C_SPL` joins between blocks
and 1000 segments per block are not yet verified on KSS 8.7.9), the real timing, and the screw
calibration.

**The recipe's own warnings** (from EasyCAD, about the part rather than the robot): *the socket
sticks out above the shade* (the grip is 20 mm below the top and the E27 body is 55 mm tall:
set the grip deeper if it should be hidden), and *the arm climbs steeper than 15° in places*
(the line between the wall and the wings: an upright nozzle may drag on the layer below there).

## Files

| File | |
|---|---|
| `straight-2-robot.easycad` | the project (the owner's `straight-2`, printer KUKA KR50, robot tilt off) |
| `Full/STRAIGHT2.SRC`, `.DAT`, `.zip` | the whole print's program |
| `Full/Robot_Simulation.html` | the whole print animated in the handover's viewer |
| `Full/collision_report.json` | the simulation's checks |
| `Full/program.json` | the program's numbers |
| `Test_40mm/…` | the same for the first 40 mm |
| `MANIFEST.sha256` | checksums of the files above |

## Rebuild

In EasyCAD (from the `claude/amazing-newton-9qraf8` branch until it's merged): open
`straight-2-robot.easycad`, Print tab, printer KUKA KR50, **Robot program** (set **Stop at this
height** 40 for the test). Then, from the repository root:

```sh
python3 -m pip install -r SPHR_Full_Print_Handover-2/Globe_E27_30_Layers/requirements.txt
python3 scripts/robot-check.py STRAIGHT2.SRC --width 5 --output collision_report.json
python3 scripts/robot-viewer.py STRAIGHT2.SRC --title "Straight 2" --output Robot_Simulation.html
```
