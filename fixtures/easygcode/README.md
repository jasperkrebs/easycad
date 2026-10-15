# EasyGCode golden fixtures

G-code written by EasyGCode itself, for EasyCAD's golden tests
(`crates/core/src/golden_tests.rs`, and the browser smoke test for `sdisc-flow`).

- `<case>.json`: the EasyGCode settings of the case.
- `<case>.gcode`: what EasyGCode's own generator (`gcode.js`) writes for them.
- `SOURCE`: the EasyGCode commit they came from.

| Case | What it covers |
|---|---|
| `sdisc-flow` | The spiral disc with EasyGCode's defaults in volumetric flow mode (20 mm³/s): EasyCAD's template as it ships |
| `sdisc-default` | The spiral disc with EasyGCode's default settings at a set speed, Klipper pellet printer |
| `sdisc-filament` | A small two-layer disc for the filament printer (P1P start/end, E in mm of filament, multiplier, fan) |
| `sdisc-bare` | Three layers without start/end blocks, falloff only inside the ring |
| `lamp-cylinder` | The lampshade generator with a straight "cone" whose ends are the same size: a cylinder in vase mode (ramped first turn, 40 whole layers, flat closing turn), for `vase_spiral` |
| `lamp-cylinder-flow` | The same cylinder in flow mode (8 mm³/s) |

Regenerate them from a clone of EasyGCode (which is only read, never changed):

```sh
node scripts/easygcode-fixtures.cjs ../easygcode
```

The tests compare commands, not comments, so EasyCAD's own header comments may differ.
