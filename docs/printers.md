# Printers

EasyCAD writes G-code for two machines, and a KUKA robot program (KRL) for a robot arm. Both
drive real machines, so this page says what EasyCAD knows about each one, where that comes from,
and what it does with it.

| Printer | Recipe line | Kind | Start and end G-code |
|---|---|---|---|
| **Ginger Additive G1** | `printer "Ginger G1"` (older projects: `"Klipper pellet"`, same printer) | pellets, Klipper; E in mm³ | EasyGCode's, calling Ginger's own macros |
| **Bambu P1P** | `printer "Bambu P1P 0.4"` | filament, 0.4 mm nozzle; E in mm of filament | EasyGCode's |
| **KUKA KR50 (SPHR cell)** | `printer "KUKA KR50"` | pellets on a robot arm; a KRL program (`.SRC` + `.DAT` in a ZIP), extruder over EthernetKRL | the SPHR handover's (below) |

The settings live in `web/js/print.js` (the Print tab, and the agent's `set_printer`); the
heater limits also go to the engine (`gcode::Pellet`), which refuses G-code that would ask a
printer for more than it takes.

## The Ginger Additive G1

A pellet (FGF) printer running Ginger's fork of Klipper on a Raspberry Pi (G1OS, a MainsailOS
fork). Everything here comes from **Ginger's wiki, https://wiki.gingeradditive.com**, whose
figures come from the machine's printer.cfg and Ginger Slicer's shipped profiles. Pages used:
Specifications, Multi-Heating Zones, Nozzle set, Max Volumetric Speed, Nozzle and barrel clog,
Material profiles, FGF-specific settings, Pressure Advance tuning, Bed heater, Bed leveling
system, G1OS software stack, Rotation Volume calibration, Main extruder motor, PLA, Error
messages, Klipper MCU communication lost.

### What EasyCAD uses, and how

| | The G1 (wiki) | In EasyCAD |
|---|---|---|
| Build volume | 1000 × 1000 × 1000 mm (X travels to 1100, Z to 1050) | the default bed, and the bed check |
| Where prints go | the bed's temperature sensor is in the middle of the plate (around X500 Y500): a large solid bottom right over it can make it read low, and the bed overshoots into its 110 °C cutoff | the recipe origin is the bed's centre (X500 Y500), taken from the bed size: change the bed's width or depth and the origin follows. "Recipe origin at X/Y" still overrides it (an origin moved by hand stays put when the bed changes; a setting saved with EasyGCode's old X125 Y125 counts as unset). The Bambu P1P keeps EasyGCode's X125 Y125 |
| Nozzles | 1, 1.8, 3, 5 and 8 mm. Ginger's profiles: layers about half the nozzle (0.6, 0.9, 1.3, 2.5, 4 mm), lines a little wider than it (1.2, 1.9, 3.2, 5.5, 9.5 mm); thicker layers drag the nozzle on the layer below | not a printer setting: the owner picks the nozzle per project (mostly the 5 mm) and can say so in the project's notes; the agent knows Ginger's table |
| Flow | Ginger's profiles run 200 mm³/s; the 1 mm nozzle tops out around 150, 3 mm around 300, 8 mm around 500. **Below about 150 mm³/s (really around 100) the barrel inlet can plug**: the passing pellets are what cool it, so the fix is a cooler upper zone, never a hotter one | max flow 200 (a material preset sets its own); a **low-flow check** that passes once the upper zone runs at least 10 °C below the other two |
| Heating zones | three: upper (feeding, Klipper's `extruder`, T0), middle (melting, `extruder1`), lower (nozzle, `extruder2`); 160–280 °C (`min_extrude_temp`, `max_temp`) | zone settings limited to 160–280 °C; the engine refuses G-code with a zone outside that, machine events included (printing on after cooling below 160 stops the print) |
| Bed | up to 110 °C (watermark control, ±1 °C) | bed setting 0–110 °C; the engine refuses more |
| Motion | 300 mm/s at most (prints at 120–150, travels up to 300), acceleration 1500 mm/s² | travel speed limited to 300 mm/s |
| E | cubic millimetres: `filament_diameter` 1.1284 mm makes the "filament" 1 mm² | the pellet mode writes E in mm³ |
| Part fan | keep it under about 70 % (the wiki's advice for filled grades; its error page lists the heating error for any): full cooling can pull the nozzle zone into a heating error mid-print | a check, of the setting and of the recipe's own fan steps |
| Pressure advance | 0 for vase mode (otherwise a repeating texture on the wall); 0.2–0.7 otherwise, smooth time 0.5 s (Ginger's fork allows up to 5 s, stock Klipper 0.2 s) | a check for vase-mode prints (one continuous line over several layers) |
| Tiny segments | G-code with thousands of very short moves stops the G1 with "stepper too far in past" | the agent knows to raise the recipe's chord tolerance |
| Materials | Ginger Slicer's shipped profiles (below) | **material presets**: the three zones, the bed and the max flow in one go (Print tab, or `set_printer` with `material`) |

Ginger Slicer's material profiles (zones top to bottom, bed, max flow):

| Material | Upper | Middle | Lower | Bed | mm³/s |
|---|---|---|---|---|---|
| Generic PLA, FormFutura PLA | 200 | 200 | 200 | 50 | 200 |
| SmartMaterials PLA | 185 | 185 | 185 | 50 | 200 |
| Azure PLA | 215 | 210 | 210 | 60 | 200 |
| Ginger rPLA | 205 | 200 | 200 | 55 | 180 |
| colorFabb FOAM PLA | 185 | 210 | 210 | 70 | 500 |
| Generic PETG | 220 | 240 | 240 | 80 | 200 |
| Azure PETG | 210 | 210 | 210 | 80 | 250 |
| FormFutura rPETG | 215 | 235 | 235 | 80 | 200 |
| FormFutura rPETG 10GF | 205 | 225 | 225 | 80 | 200 |
| FormFutura rPETG 20GF, Generic PETG GF | 220 | 235 | 245 | 80 | 200 |
| Polymaker PETG 10GF | 190 | 230 | 240 | 80 | 200 |
| Polymaker PETG 20GF | 220 | 230 | 240 | 80 | 200 |
| Generic PP | 170 | 240 | 230 | 80 | 200 |
| Generic ABS / ASA | 220 | 240 | 260 | 80 | 200 |
| Generic HIPS | 210 | 230 | 230 | 80 | 200 |

Ginger supports the G1 on PLA and PETG; the others are profiles it ships without
characterising them. The datasheet of the grade in the hopper is the authority.

### The start and end G-code on the G1

EasyCAD writes EasyGCode's start block unchanged (CLAUDE.md: G-code safety); only its settings
change its values. Besides the temperatures, pressure advance and purge amount, the Print tab
has (23 Nov, the owner's ask; EasyGCode's values by default, so the G-code is the same until one
is changed; each kept within a range, out of it the G-code is refused): **screw rotation
distance** (456, step 5), **mixing stepper rotation distance** (8000, step 6), **mixing
multiplier** (1), **purge length** (400) and **speed** (500, step 7), **parking purge layer
height** (2) and **speed** (10000, step 4), **pressure advance smooth time** (0.5 s, step 7).
What each step does on the G1:

1. `SET_PRESSURE_ADVANCE … SMOOTH_TIME=0.04`, `ADVANCE=0.0`: pressure advance off while heating
   and purging.
2. `_GINGER_BUZZER_TONE_INITIAL`, `_GINGER_BED_HEATING`, `_GINGER_EXTRUDER_SET_UP/MID/DOWN`:
   Ginger's internal macros (in every G1's shared configuration, `G1-Configs/macro.cfg`).
   The wiki calls them steps of its own macros, not meant to be run by hand; EasyGCode's prints
   have run on them.
3. `G28`, `BED_MESH_PROFILE LOAD=global`: homes and loads the **saved** bed mesh. Ginger Slicer
   instead sends `START_PRINT … KAMP_LEVELING=1`, which probes fresh over the part. So with
   EasyCAD (and EasyGCode) G-code, **run `BED_LEVEL` after every nozzle change, after moving
   the machine, and monthly** (the wiki's list); the Print tab says so too.
4. `_GINGER_PURGE_PARKING …`, the waits for the zones and the bed (`_GINGER_EXTRUDER_WAIT_*`,
   `_GINGER_BED_WAIT` from 10 °C under to 40 °C over the bed's target),
   `_GINGER_EXTRUDER_MIXING_MULTIPLIER S=1`.
5. `SET_EXTRUDER_ROTATION_DISTANCE EXTRUDER=extruder DISTANCE=456`: the screw's Rotation Volume,
   fixed at 456 mm³ per turn. The wiki calibrates it per material (PLA about 500, PETG about
   600). Here, calibrate with the **extrusion multiplier** instead: new multiplier = current
   multiplier × target width ÷ measured width (measure the first layer's height first: a wrong
   Z offset looks like wrong flow).
6. `SET_EXTRUDER_ROTATION_DISTANCE EXTRUDER=mixing_stepper DISTANCE=8000`: the hopper auger at
   the standard 8000 (the wiki: 4000 for sticky materials or a lot of masterbatch, 12000 for
   free-flowing ones).
7. `_GINGER_PURGE PURGE_LENGHT=400 …` (the macro's own spelling), `G90`, `G92 E0`, `M83`,
   `M220 S100`, `M221 S100`, pressure advance from the settings with smooth time 0.5 s, fan off,
   a beep.

The end block lifts clear of the print (never past the build height), turns the heaters and
the fan off and the motors off. Ginger's own `END_PRINT` also sets pressure advance back to 0;
here the next print's start block does.

Machine events (recipe steps): `temperature` writes `_GINGER_EXTRUDER_SET_UP/MID/DOWN S=…`;
`wait_temperature` writes `TEMPERATURE_WAIT SENSOR=extruder2 MINIMUM=…` (hot: the nozzle zone
within 2 °C) or `SENSOR=extruder`, `extruder1`, `extruder2` `MAXIMUM=…` (cool: every zone);
`speed_override` and `extrusion_override` write `M220`/`M221`. These are EasyGCode's commands
(its bend stool's foaming), golden-tested against it.

**The automatic feeder** (the wiki: `START_PRINT` switches FEEDER, the vacuum pellet feeding,
on; EasyGCode's start block doesn't call `START_PRINT`): not in use. The owner pours the pellets
into the hopper by hand (18 Oct). If the vacuum feeding comes back, check that a print switches
it on.

**Open question**, to settle against the machine or its configuration files:
- Klipper's `max_extrude_cross_section` and `max_extrude_only_distance` (printer.cfg) stop a
  print whose bead or purge is over them. EasyGCode's prints run, so its sizes are fine; bigger
  beads (the 8 mm nozzle's 9.5 × 4 mm) are worth checking against the file.

### When printer.cfg arrives

Add it as `printers/ginger-g1/printer.cfg` (and, if you can, the shared layer `G1-Configs/`:
`main.cfg` with `START_PRINT`/`END_PRINT`, `macro.cfg` with the `_GINGER_` macros). Then check
these against it:
- `[extruder]` (and `extruder1`, `extruder2`) `max_temp`, `min_extrude_temp` → the engine's
  `Pellet::max_temp`, `min_extrude_temp`, and `G1.limits` in `web/js/print.js`;
- `[heater_bed] max_temp` → `max_bed`;
- `[stepper_x]`, `[stepper_y]`, `[stepper_z]` `position_max`, and `[bed_mesh]` → the bed size;
- `[printer] max_velocity`, `max_accel` → the travel speed limit, the time estimate;
- `[extruder] filament_diameter` (1.1284), `max_extrude_cross_section`,
  `max_extrude_only_distance`, `pressure_advance_smooth_time`;
- `macro.cfg`: what each `_GINGER_` step of the start block does (and whether one switches the
  feeder on, for when it's used again).

## Another Klipper printer

What's specific to a printer is data: the bed, the nozzles, the setting ranges, the heater
limits (which the engine checks), the material presets. What only the G1 has is the G-code
that calls Ginger's macros: the start and end blocks and the zone commands of the machine
events. Another Klipper printer doesn't have those macros (Klipper answers "Unknown command",
so nothing heats), and stock Klipper refuses the start block's pressure-advance smooth time of
0.5 s (its limit is 0.2 s).

So a generic Klipper printer needs its own start and end G-code. The usual way is to call the
printer's own `START_PRINT`/`END_PRINT` (or `PRINT_START`/`PRINT_END`) macros with the
temperatures, and let them home, heat and purge. That's a new start block, so it ships only
with the owner's OK and a hand-checked fixture (CLAUDE.md, G-code safety).

## The Bambu P1P

A filament printer (1.75 mm, a 0.4 mm nozzle, a 256 × 256 × 256 mm bed), with EasyGCode's
Marlin-style start and end blocks. Its hotend manages about 20 mm³/s with PLA (the max-flow
check). Line widths between 1 and 2 times the nozzle pass the check.

## The KUKA KR50 (the SPHR cell)

A KUKA KR 50 R2500 (KRC5, KSS 8.7.9) carrying a pellet extruder, on a 1200 × 2800 mm table. Its
extruder, heaters and fans are driven over EthernetKRL (`EKI_SEND` to `EthernetKRL_Config`:
`START_EXTRUDE`, `STOP_EXTRUDE`, `CHANGE_SPEED1 <screw command>`, `SET_TEMP T0..T2`,
`SET_FAN1/2`). Everything here comes from the **SPHR handover** in the repo,
`SPHR_Full_Print_Handover-2/` (20 Nov 2026): its `HANDOVER.md`, its 30-layer test program
(`Globe_E27_30_Layers/SPHR.SRC`, `SPHR.DAT`) and the scripts that build it (`build_spline.py`,
`Globe_E27_Smoothed_Roll/reference/build_trumpet.py`, `kinematics.py`). The handover's programs
were checked in its own simulation (collisions, joint limits), not yet on the controller.

The writer is `easycad_core::krl`; the Print tab's printer **KUKA KR50** (`web/js/print.js`,
`KR50`) holds the cell's settings. The program it writes is the handover's, line for line where
they do the same thing:

1. **Start:** frames from the DAT (`TR_TOOL`, `TR_BASE`), the load (`LOAD_DATA[6]` with the
   tool's mass and centre of mass), orientation limits, `$IPO_MODE = #BASE`, `$ORI_TYPE = #VAR`;
   heaters T0 220, T1 190, T2 70 °C, part fan off, firecap fan 5; **HALT** (heat up).
2. **Purge:** to the purge point (world 1088.30, 134.86, −590) upright, **HALT**, `WAIT SEC 0`,
   screw 12000 for 20 s, then the print's first command; to 5 mm above the first point at
   100 mm/s and down onto it at 20 mm/s.
3. **The print:** the spline restrictions on (orientation speed and acceleration, Cartesian
   acceleration; the old ones saved), `SPLINE WITH $ACC={CP 0.050, ORI1 150.0, ORI2 150.0}`,
   one `SPL` per kept point with its own `$VEL.CP`; extruder changes and the part fan as
   `TRIGGER WHEN PATH=0 ONSTART` calling a small subroutine each (`<PROGRAM>_EVT_n`).
4. **End:** `WAIT SEC 0`, extruder stopped, screw 0, heaters 0; lift 50 mm, the nozzle turned
   upright at 30 °/s, fans off, the spline settings put back.

What's EasyCAD's own:

| | In EasyCAD |
|---|---|
| Where | each point is the recipe origin's world position (`robot.origin`: the handover's print centre, its z the measured bed −659) plus the toolpath point, written less `TR_BASE`. The Print tab's bed (1200 × 2800, the origin at 375, 1750 on it) is the cell's table: every point must be over it |
| Nozzle | along the toolpath's `axis` (the `tilt` step; upright without it), with the handover's roll: world −Y kept across the nozzle. At most 60° from upright, or the program is refused |
| Speed | each move's own (at a set flow, the flow ÷ its bead), at most 25 mm/s (`robot.maxSpeed`; the handover peaks at 21.3) and slow enough that the wrist turns at most 30 °/s (`robot.maxTurn`), to 0.1 mm/s rounded down |
| Extruder | screw command = `robot.screw` (30) × mm³/s laid down (bead × extrusion × speed × multiplier), changed only when it moves more than 2 %. 30 comes from the handover's wall (300 per mm/s with a 4.5 × 2.5 mm bead, 9.91 mm²): **calibrate it** with a measured purge |
| Points | within a stretch of one speed and command, a point is left out where the straight line past it stays within `robot.tolerance` (0.2 mm) and the nozzle within 0.5°; the spline runs smoothly through the rest |
| Blocks | at most `robot.block` (1000) segments per SPLINE block, joined with `ENDSPLINE C_SPL` |
| Joints | every pose (the purge, the approach, each kept point and three between each two, the lift) is worked out on the handover's kinematics (`kuka.rs`, a port of its `kinematics.py`, tested against it): out of reach, or a joint (A1–A6, A1, A4 and A6 counted on continuously from the handover's start pose) closer than 3° to its limit, and the program is refused |
| Tool roll | the handover's roll keeps world −Y across the nozzle; the tool is mounted at 90° to the flange, so that roll decides how far the wrist (A5) bends. Where a joint would come within `robot.wristMargin` (10°) of its limit, the tool turns about the nozzle (the roll nearest the handover's that keeps the margin, up to ±90°, changing at most `robot.rollRate`, 1.5 °/mm of path) |
| Tilt relief | where even rolling can't keep a joint 3° inside its limit, the tilt is eased back there (to 75 % at the point, blending back over 40 mm each way) until it can, as the handover's own "local tilt relief". The bead isn't recomputed for the eased tilt (a little thinner along the nozzle there) |
| Test print | `robot.stopAt` (mm): stops at the first point above it, then the normal end. The handover's short test is 30 layers |

**Not on the robot yet** (the program is refused): travel moves (it prints one continuous line),
and the recipe's machine events (fans, pauses, temperatures).

**To check on the controller first:** that it compiles (KSS 8.7: the `C_SPL` between blocks,
and whether one SPLINE block may hold 1000 segments: KUKA's manual for the exact version is the
authority); reach and joint limits (EasyCAD doesn't check them yet: the handover's
`kinematics.py` and `rough_checks.py` do, for its globe); the tool's clearance to the bed and the
print (`scripts/robot-check.py` replays the program on the handover's model, below); and the screw
calibration.

### Printing on the robot

A recipe ends its toolpath with `tilt(max: 45 deg, smooth: 20 mm, bed: 60 mm)`
([definition-language.md](definition-language.md)): the nozzle along the wall (from the bead
underneath), averaged over 20 mm of path so the wrist turns gently, at most 45° and less near
the bed. Tilted 45°, the tool's lowest point (a nozzle fan, the handover's proxies, at the worst
roll) hangs about 42 mm below the nozzle tip; growing the tilt evenly from upright on the bed to
45° at 60 mm keeps every part but the nozzle at least 18 mm off the bed (55 mm is the least that
keeps the handover's 10 mm). Keep overhang compensation by layer height on: on a Cartesian printer it squeezes a
leaning wall's layers to the layer height × cos(lean), which is the same as stepping one layer
along the wall; `tilt` then gives each move its full bead back along the nozzle. The Thread
pendant has this as its **Robot** setting.

### Checking a program on the handover's model

`scripts/robot-check.py` replays a program EasyCAD wrote on the handover's own robot model, as
the handover's `rough_checks.py` does for its globe (which reads its replay from the globe's
HTML and models the printed material from the globe's profile, so it can't take another print):

```sh
python3 -m pip install -r SPHR_Full_Print_Handover-2/Globe_E27_30_Layers/requirements.txt
python3 scripts/robot-check.py EASYCAD.SRC --width 5 --output report.json
```

Every LIN and SPL target, and three samples between each two (straight, and slerp for the
nozzle), are turned into joint angles with the handover's `kinematics.py`; then: joint limits
(A1–A5) and reach; tool against arm, arm against itself and everything against the pedestal
(`proxy_collision.py`, `cad_proxy.json`, the fan housings from `fan_pair_coordinates.json`, the
heater barrel left out as the handover does); every part but the nozzle at least 10 mm off the
bed; and every tool part and the wrist against the material already printed: the program's own
bead (its targets every 2 mm, a round bead of `--width`), leaving out the last 20 mm behind the
nozzle; the fans should keep 5 mm. It exits non-zero on a problem. Like the handover's, it
doesn't see the controller's spline, cables, fixtures or the start pose. `--stride` checks the
material on every Nth sample (a whole print's program takes long).

To **watch** a program, `scripts/robot-viewer.py` replays it the way the handover's
`build_trumpet.py` replays its own and fills the handover's viewer
(`Globe_E27_Smoothed_Roll/reference/viewer_template.html`): the arm, the tool and the print as it
grows, play and scrub, "Whole robot" or the print area, layer by layer, with the tool-to-bed
readout. Nominal, as the handover's: straight at the programmed speed between targets; layers are
counted by turns around the print's centre.

```sh
python3 scripts/robot-viewer.py EASYCAD.SRC --title "Sphere: first 40 mm" --output sphere.html
```

