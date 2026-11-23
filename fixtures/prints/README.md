# Approved prints

Prints the owner made and was happy with: each project file as saved from EasyCAD (its recipe,
its settings, and its printer's settings in the `#~ printer` line) and the G-code it makes
(`name.gcode`; a project with several parts: `name.gcode`, `name-1.gcode`, …).

`scripts/prints.test.mjs` (CI runs it on the engine it just built) checks that the engine still
makes exactly that G-code, so a print that worked never changes by accident. When a change is
meant, write the new G-code with `UPDATE_PRINTS=1 node --test scripts/prints.test.mjs` and say so
in the pull request; the owner approves it.

| Print | What | Printer |
|---|---|---|
| `test-8` | The overhang test that printed very well (23 Nov 2026): a bell shade on E14, overhang compensation by layer height at 86 %, steep curve 0.85, pivot 73° (every template's default since). Made from the owner's test 7 file with test 8's three values; the printer settings are the P1P's defaults until the owner saves it again from the app. | Bambu P1P, 0.8 mm nozzle, 5 mm line, flow 15 |
