# SPHR full-print handover

Revision dated 20 November 2026. This package captures the current E27 globe print and its 30-layer test for the KUKA KR 50 R2500, KRC5, and KSS 8.7.9.

## Start here

- For the full print, use `Globe_E27_Full/SPHR.zip`. For the short test, use `Globe_E27_30_Layers/SPHR.zip`. Each controller ZIP contains `SPHR.SRC` and `SPHR.DAT`.
- Extract the handover to open `Globe_E27_Full/Non_Planar_Demo.html` in a browser. Choose **Whole robot** under **Frame** to see the pedestal. The 30-layer folder also has its own preview.
- To use this handover in Claude, ChatGPT, or Codex, attach this ZIP. If the app cannot extract ZIPs, extract it first and attach `HANDOVER.md`, the matching `SPHR.SRC`, `SPHR.DAT`, `QA.json`, and `collision_report.json`. Include the HTML if you want the app to inspect the simulation.

Example prompt to copy

> Read HANDOVER.md as project context. Use Globe_E27_Full/SPHR.SRC and SPHR.DAT as the current baseline. Check QA.json and collision_report.json against that source. Explain my requested change before regenerating the files, and tell me what you verified in the simulation and what requires the controller.

## Verified configuration

| Item | Current value |
|---|---|
| Full path | 90 layer markers, 192 mm profile height, one level extrusion taper turn, then one dry wipe turn |
| Short test | Completes layer 30, stops extrusion and heat, lifts 50 mm, returns upright, then shuts off both fans |
| E27 throat | 41.5 mm nominal internal bore based on a 4.5 mm bead. The nozzle-centreline diameter is 46.0 mm, the straight section is 20 mm, and the spiral pitch is 2.5 mm measured along the profile |
| Globe | Inferred R95 profile with a cubic throat blend. Generated lip nozzle-centreline diameter 154 mm |
| Brim | Six rings, 3.9 mm centre-to-centre spacing. Innermost ring 3.9 mm and outermost ring 23.4 mm radially from the spiral centreline. Connected seams are on world +Y. Each ring has a 10 mm entry trim |
| Bed and start | Measured bed world Z = -659 mm. Brim Z = -658 mm. The spiral starts 0.15 mm above the bed |
| World placement | Print centre X = 1088.304936 mm, Y = -115.143158 mm. Bed X spans 713.304936 to 1913.304936 mm and Y spans -1865.143158 to 934.856842 mm. Subsection seams are at Y = -1165.143158, -465.143158, and 234.856842 mm |
| Tool and base | TR_TOOL XYZ (-508.00, 3.50, 139.34) mm, ABC (-180, -90, 0) deg. TR_BASE XYZ (0, 50, -0.30) mm, ABC (0, 0, 0) deg |
| Load | 18 kg with estimated flange-relative centre of mass (-90.65, 5.95, 164.92) mm. The program initializes from LOAD_DATA[6] before applying mass and centre-of-mass overrides and inherits its inertia |
| Pedestal | Fixed cuboid, approximately 611 x 480 x 800 mm, from world Z = -800 to 0 mm. Footprint derived from the lowest static base section. Exact bounds are in collision_report.json |
| Orientation | Smoothed roll projects world -Y into the plane normal to the nozzle. Tilt caps at 45 deg. Sampled maximum 40.55 deg. A6 is modeled as continuous and swept about -92.5 to +42.0 deg in the full replay |
| Motion | Brim at 10 mm/s. Layers 1-15 run at 6 mm/s, with peak speed at 21.3 mm/s on layers 64-67. Final extrusion taper and dry wipe at 16.2 mm/s. Per-layer speed commands are in QA.json |
| Limits and motion type | KRL SPLINE for the wall and taper. LIN for the approach and brim. Orientation limits 200 deg/s and 150 deg/s squared during printing. The spline uses 0.050 m/s squared Cartesian acceleration, with spline Cartesian and orientation restrictions enabled and prior settings restored on exit |
| Screw schedule | Brim steady command 1000. Final 10 mm commands 836, 672, 508, 344, then 180 for the join. Layer 1 starts at 180. Layer 2 starts at 1620 (90 percent at 6 mm/s). Layers 2-9 use 1620, 1646, 1671, 1697, 1723, 1749, 1774, 1800. Later steady wall commands scale at 300 screw units per mm/s, peaking at 6390 |
| Heat and fans | EthernetKRL_Config. Heater targets T0 220 C, T1 190 C, firecap T2 70 C. Firecap fan command 5. Part fan command 255 from the first wall turn |
| Startup and purge | First HALT after the heat and initial fan commands. The next Start moves to the purge point at world (1088.30, 134.86, -590.00) mm. Second HALT at the purge point, followed by WAIT SEC 0 before the purge commands. The next Start purges at screw command 12000 for 20 s. Transfer at 100 mm/s and approach at 20 mm/s |
| Full finish | Taper extrusion at the lip, stop the screw and set all heater targets to zero. Leave both fans on through the same 60-pose dry wipe, lift 50 mm vertically, and turn upright in place. After the final move and WAIT SEC 0, switch off both fans |

## Simulation and validation

- Full nominal runtime **40 min 41 sec**. The 30-layer nominal runtime is **13 min 20 sec**. Operator pauses and heat-up time are outside those estimates.
- The full replay checked **91,413 poses**, including **1,164 finish poses**. It reported zero modeled collisions and zero joint-limit violations. Minimum fan clearances were 11.82 mm to the bed and 6.84 mm to the printed material. The pedestal-to-moving-part separation had a conservative lower bound of 170 mm. The nearest sampled joint limit was A5, with 1.65 deg of margin.
- The 30-layer replay checked **31,226 poses** with the same pedestal and reported zero modeled collisions or joint-limit violations.
- The replay uses simplified CAD and an offline spline estimate. The collision scope excludes the heater barrel, expected nozzle contact, the move from the actual startup pose, cell fixtures, cable and hose routing, and controller interpolation. Controller compilation and motion timing, actual cell clearances, and sustained extrusion at the assigned screw commands need physical verification. The packaged collision reports identify their exact KRL by SHA-256.

## Rebuild from the extracted handover

A coding assistant can reproduce the current build using Python with NumPy and Shapely. Run these from the extracted ZIP root.

```text
python -m pip install -r Globe_E27_30_Layers/requirements.txt
python Globe_E27_30_Layers/build_spline.py --full
python Globe_E27_30_Layers/rough_checks.py --output Globe_E27_Full
```

For the 30-layer test, run `python Globe_E27_30_Layers/build_spline.py` and then `python Globe_E27_30_Layers/rough_checks.py`. Keep the three sibling folders together. The `Globe_E27_Smoothed_Roll` folder carries the speed plan, spline reference positions, kinematics inputs, and viewer template used by the rebuild. Use the applicable `SPHR.zip` for controller import.

`QA.json` records the generated KRL and G-code hashes. `MANIFEST.sha256` covers the handover's packaged files. ZIP container hashes can change when rebuilt because of ZIP timestamps.
