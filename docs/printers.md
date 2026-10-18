# Printers

EasyCAD writes G-code for two machines. G-code drives real machines, so this page says what
EasyCAD knows about each printer, where that comes from, and what it does with it.

| Printer | Recipe line | Kind | Start and end G-code |
|---|---|---|---|
| **Ginger Additive G1** | `printer "Ginger G1"` (older projects: `"Klipper pellet"`, same printer) | pellets, Klipper; E in mm³ | EasyGCode's, calling Ginger's own macros |
| **Bambu P1P** | `printer "Bambu P1P 0.4"` | filament, 0.4 mm nozzle; E in mm of filament | EasyGCode's |

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
| Where prints go | the bed's temperature sensor is in the middle of the plate (around X500 Y500): a large solid bottom right over it can make it read low, and the bed overshoots into its 110 °C cutoff | the recipe origin stays at EasyGCode's X125 Y125; a print bigger than about 250 mm needs it moved (the bed check says so) |
| Nozzles | 1, 1.8, 3, 5 and 8 mm. Ginger's profiles: layers about half the nozzle (0.6, 0.9, 1.3, 2.5, 4 mm), lines a little wider than it (1.2, 1.9, 3.2, 5.5, 9.5 mm); thicker layers drag the nozzle on the layer below | the **nozzle** setting (the one in the machine; 1.8 mm to start, matching EasyGCode's 2 mm lines), and checks of line width and layer height against it |
| Flow | Ginger's profiles run 200 mm³/s; the 1 mm nozzle tops out around 150, 3 mm around 300, 8 mm around 500. **Below about 150 mm³/s (really around 100) the barrel inlet can plug**: the passing pellets are what cool it, so the fix is a cooler upper zone, never a hotter one | max flow 200 (a material preset sets its own); the nozzle's ceiling; a **low-flow check** that passes once the upper zone runs at least 10 °C below the other two |
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
(temperatures, pressure advance, purge) change it. What each step does on the G1:

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
   free-flowing ones; not a setting yet).
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

**Open questions**, to settle against the machine or its configuration files:
- **The automatic feeder.** The wiki says `START_PRINT` switches FEEDER (the vacuum pellet
  feeding) on; EasyGCode's start block doesn't call `START_PRINT`. If EasyGCode prints refill the
  hopper, one of the purge macros switches it on; if not, switch FEEDER on before printing (and
  tell the next session, so the start block can say so).
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
- `macro.cfg`: what each `_GINGER_` step of the start block does (the feeder question above).

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
